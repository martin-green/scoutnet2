//! Kompatibilitet med dagens Scoutnet-API (planen 6.4), så att befintliga integrationer som Skojjt
//! kan flytta utan omskrivning: `GET /api/group/memberlist` med samma `{value, raw_value}`-format.
//!
//! Nycklarna är per kår, lagras bara som hash och har behörigheter (scopes). Personnummer och
//! kontaktuppgifter lämnas bara ut om nyckeln uttryckligen har fått det. Varje hämtning loggas.

use std::collections::HashMap;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sn_domain::{Term, TroopType};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::{audit, bodies::load_visible_body};
use crate::{
    AppState,
    auth::{CurrentUser, hash_token},
    authz::{Permission, role_label},
    error::{ApiError, ApiResult},
};

const SCOPES: [&str; 3] = ["memberlist", "memberlist:contact", "memberlist:ssno"];

// ---------------------------------------------------------------------------------------------
// Hantering av API-nycklar

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct ApiKeyInfo {
    pub id: Uuid,
    pub name: String,
    pub scopes: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_by: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreatedApiKey {
    pub info: ApiKeyInfo,
    /// Nyckeln. Visas bara nu och kan inte hämtas igen.
    pub key: String,
    /// Användarnamn vid Basic-autentisering (kårens nummer, som i Scoutnet).
    pub username: String,
    pub endpoint: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateApiKey {
    pub name: String,
    /// `memberlist` (krävs), `memberlist:contact`, `memberlist:ssno`.
    pub scopes: Vec<String>,
}

async fn load_group(state: &AppState, user: &CurrentUser, key: &str) -> ApiResult<(Uuid, String, i32)> {
    let (id, kind, path) = load_visible_body(state, user, key, Permission::MembersRead).await?;
    if kind != "group" {
        return Err(ApiError::BadRequest("API-nycklar skapas för en kår".into()));
    }
    if !user.0.can(Permission::MembersManage, &path) {
        return Err(ApiError::Forbidden);
    }
    let number: i32 = sqlx::query_scalar("SELECT number FROM body WHERE id = $1").bind(id).fetch_one(&state.db).await?;
    Ok((id, path, number))
}

const KEY_SELECT: &str = "SELECT k.id, k.name, k.scopes, k.created_at, k.last_used_at,
        p.first_name || ' ' || p.last_name AS created_by
 FROM api_key k LEFT JOIN account a ON a.id = k.created_by LEFT JOIN person p ON p.id = a.person_id";

/// Kårens aktiva API-nycklar. Kräver `members_manage`.
#[utoipa::path(get, path = "/v1/bodies/{id}/api-keys", tag = "integrations",
    params(("id" = String, Path, description = "Kårens id eller nyckel, t.ex. `group-1001`")),
    responses((status = 200, body = [ApiKeyInfo]), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn list_keys(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(key): Path<String>,
) -> ApiResult<Json<Vec<ApiKeyInfo>>> {
    let (group_id, ..) = load_group(&state, &user, &key).await?;
    let keys = sqlx::query_as::<_, ApiKeyInfo>(sqlx::AssertSqlSafe(format!(
        "{KEY_SELECT} WHERE k.body_id = $1 AND k.revoked_at IS NULL ORDER BY k.created_at"
    )))
    .bind(group_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(keys))
}

/// Skapar en API-nyckel för kåren. Personnummer kräver behörigheten `national_id_read`.
#[utoipa::path(post, path = "/v1/bodies/{id}/api-keys", tag = "integrations",
    params(("id" = String, Path, description = "Kårens id eller nyckel")),
    request_body = CreateApiKey,
    responses((status = 201, body = CreatedApiKey), (status = 400, body = crate::error::Problem), (status = 403, body = crate::error::Problem)))]
pub async fn create_key(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(key): Path<String>,
    Json(req): Json<CreateApiKey>,
) -> ApiResult<(StatusCode, Json<CreatedApiKey>)> {
    let (group_id, path, number) = load_group(&state, &user, &key).await?;
    let name = req.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(ApiError::BadRequest("namnet ska vara 1–80 tecken".into()));
    }
    let mut scopes: Vec<String> = req.scopes.clone();
    scopes.sort();
    scopes.dedup();
    if let Some(bad) = scopes.iter().find(|s| !SCOPES.contains(&s.as_str())) {
        return Err(ApiError::BadRequest(format!("okänd behörighet: {bad}")));
    }
    if !scopes.iter().any(|s| s == "memberlist") {
        scopes.insert(0, "memberlist".into());
    }
    // Ingen kan ge en integration mer än man själv får se.
    if scopes.iter().any(|s| s == "memberlist:ssno") && !user.0.can(Permission::NationalIdRead, &path) {
        return Err(ApiError::Forbidden);
    }
    if scopes.iter().any(|s| s == "memberlist:contact") && !user.0.can(Permission::MembersContactRead, &path) {
        return Err(ApiError::Forbidden);
    }
    let mut bytes = [0u8; 24];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("slumptal: {e}"))?;
    let secret = format!("sn2_{}", hex::encode(bytes));
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO api_key (body_id, name, key_hash, scopes, created_by) VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(group_id)
    .bind(name)
    .bind(hash_token(&secret))
    .bind(&scopes)
    .bind(user.0.account_id)
    .fetch_one(&state.db)
    .await?;
    audit(&state.db, user.0.account_id, "api_key.create", "body", group_id, json!({ "key": id, "scopes": scopes }))
        .await;
    let info = sqlx::query_as::<_, ApiKeyInfo>(sqlx::AssertSqlSafe(format!("{KEY_SELECT} WHERE k.id = $1")))
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(CreatedApiKey {
            info,
            key: secret,
            username: number.to_string(),
            endpoint: format!("{}/api/group/memberlist", state.config.public_url),
        }),
    ))
}

/// Återkallar en API-nyckel direkt.
#[utoipa::path(delete, path = "/v1/api-keys/{id}", tag = "integrations",
    params(("id" = Uuid, Path, description = "Nyckelns id")),
    responses((status = 204), (status = 404, body = crate::error::Problem)))]
pub async fn revoke_key(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let row: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT k.body_id, b.path::text FROM api_key k JOIN body b ON b.id = k.body_id WHERE k.id = $1 AND k.revoked_at IS NULL",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let Some((body_id, _)) = row.filter(|(_, path)| user.0.can(Permission::MembersManage, path)) else {
        return Err(ApiError::NotFound);
    };
    sqlx::query("UPDATE api_key SET revoked_at = now() WHERE id = $1").bind(id).execute(&state.db).await?;
    audit(&state.db, user.0.account_id, "api_key.revoke", "body", body_id, json!({ "key": id })).await;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------------------------
// GET /api/group/memberlist (Scoutnet-format)

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct MemberlistQuery {
    /// Kårens nummer. Alternativ till Basic-autentisering (avråds, nyckeln hamnar i loggar).
    pub id: Option<String>,
    /// API-nyckel. Alternativ till Basic-autentisering (avråds).
    pub key: Option<String>,
    /// `1` = väntelistan i stället för aktiva medlemmar.
    pub waiting: Option<String>,
    /// `1` = medlemmar som väntar på godkännande.
    pub awaiting_approval: Option<String>,
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, HeaderValue::from_static("Basic realm=\"Scoutnet 2\""))],
        Json(json!({ "error": "Felaktigt kår-id eller API-nyckel" })),
    )
        .into_response()
}

/// Kårnummer och nyckel från Basic-autentisering, annars från frågan (`id`, `key`).
fn credentials(headers: &HeaderMap, q: &MemberlistQuery) -> Option<(String, String, bool)> {
    let basic = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Basic "))
        .and_then(|b| STANDARD.decode(b.trim()).ok())
        .and_then(|b| String::from_utf8(b).ok())
        .and_then(|s| s.split_once(':').map(|(u, p)| (u.to_owned(), p.to_owned())));
    match basic {
        Some((u, p)) => Some((u, p, false)),
        None => Some((q.id.clone()?, q.key.clone()?, true)),
    }
}

#[derive(sqlx::FromRow)]
struct MemberRow {
    id: Uuid,
    member_no: i32,
    first_name: String,
    last_name: String,
    nickname: Option<String>,
    national_id: Option<String>,
    protected_identity: bool,
    birth_date: NaiveDate,
    sex: String,
    membership_status: String,
    created_at: DateTime<Utc>,
    membership_confirmed_at: Option<NaiveDate>,
    waiting_since: Option<NaiveDate>,
    group_name: String,
    troop_name: Option<String>,
    troop_number: Option<i32>,
    troop_type: Option<String>,
    patrol_name: Option<String>,
    patrol_number: Option<i32>,
    is_leader: bool,
    email: Option<String>,
    alt_email: Option<String>,
    mobile_phone: Option<String>,
    home_phone: Option<String>,
    can_help: bool,
    care_of: Option<String>,
    line1: Option<String>,
    line2: Option<String>,
    postcode: Option<String>,
    city: Option<String>,
    country_code: Option<String>,
    g1_name: Option<String>,
    g1_email: Option<String>,
    g1_mobile: Option<String>,
    g1_home: Option<String>,
    g2_name: Option<String>,
    g2_email: Option<String>,
    g2_mobile: Option<String>,
    g2_home: Option<String>,
    cur_paid: Option<NaiveDate>,
    cur_due: Option<NaiveDate>,
    cur_kid: Option<String>,
    prev_paid: Option<NaiveDate>,
}

/// Rolltilldelning: person, enhetstyp, enhetens namn, enhetens nummer, rollnyckel.
type RoleRow = (Uuid, String, String, i32, String);

fn vr(value: impl Into<Value>, raw: impl Into<Value>) -> Value {
    json!({ "value": value.into(), "raw_value": raw.into() })
}

fn put(obj: &mut Map<String, Value>, key: &str, value: Option<impl Into<String>>) {
    if let Some(v) = value {
        let v: String = v.into();
        if !v.is_empty() {
            obj.insert(key.into(), vr(v.clone(), v));
        }
    }
}

/// Scoutnets rollkoder för avdelningsroller (som Skojjt använder för att hitta ledare).
fn unit_role_code(role: &str) -> Option<i32> {
    match role {
        "avdelningsledare" => Some(2),
        "ledare" => Some(3),
        "assisterande_ledare" => Some(5),
        _ => None,
    }
}

/// Medlemslista i Scoutnets format, för befintliga integrationer. Autentisering med HTTP Basic:
/// användarnamn = kårens nummer, lösenord = API-nyckel.
#[utoipa::path(get, path = "/api/group/memberlist", tag = "integrations", params(MemberlistQuery),
    responses((status = 200, description = "`{data: {medlemsnummer: {fält: {value, raw_value}}}, labels}`"), (status = 401, description = "Fel kår-id eller nyckel")))]
#[expect(clippy::too_many_lines, reason = "fältmappningen mot Scoutnet är lättast att granska i ett stycke")]
pub async fn memberlist(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<MemberlistQuery>,
) -> ApiResult<Response> {
    let Some((group_no, secret, via_query)) = credentials(&headers, &q) else { return Ok(unauthorized()) };
    let Ok(group_no) = group_no.trim().parse::<i32>() else { return Ok(unauthorized()) };
    let key: Option<(Uuid, Uuid, Vec<String>, String)> = sqlx::query_as(
        "UPDATE api_key k SET last_used_at = now() FROM body b
         WHERE k.key_hash = $1 AND k.revoked_at IS NULL AND b.id = k.body_id AND b.kind = 'group' AND b.number = $2
         RETURNING k.id, k.body_id, k.scopes, k.name",
    )
    .bind(hash_token(&secret))
    .bind(group_no)
    .fetch_optional(&state.db)
    .await?;
    let Some((key_id, group_id, scopes, key_name)) = key else { return Ok(unauthorized()) };
    if !scopes.iter().any(|s| s == "memberlist") {
        return Ok(unauthorized());
    }
    let contact = scopes.iter().any(|s| s == "memberlist:contact");
    let ssno = scopes.iter().any(|s| s == "memberlist:ssno");
    let statuses: Vec<&str> = if q.waiting.as_deref() == Some("1") {
        vec!["waiting"]
    } else if q.awaiting_approval.as_deref() == Some("1") {
        vec!["pending"]
    } else {
        vec!["active"]
    };
    let term = Term::containing(chrono::Local::now().date_naive());

    let rows = sqlx::query_as::<_, MemberRow>(
        "SELECT mc.id, mc.member_no, mc.first_name, mc.last_name, mc.nickname, mc.national_id, mc.protected_identity,
                mc.birth_date, mc.sex, mc.membership_status, mc.created_at, mc.membership_confirmed_at, mc.waiting_since,
                mc.group_name, mc.troop_name, bt.number AS troop_number, mc.troop_type, mc.patrol_name, bp.number AS patrol_number,
                mc.is_leader, mc.email, mc.alt_email, mc.mobile_phone, mc.home_phone, mc.can_help,
                ad.care_of, ad.line1, ad.line2, ad.postcode, ad.city, ad.country_code,
                g1.name AS g1_name, g1.email AS g1_email, g1.mobile_phone AS g1_mobile, g1.home_phone AS g1_home,
                g2.name AS g2_name, g2.email AS g2_email, g2.mobile_phone AS g2_mobile, g2.home_phone AS g2_home,
                fc.paid_on AS cur_paid, fc.due_date AS cur_due, fc.kid AS cur_kid, fp.paid_on AS prev_paid
         FROM member_current mc
         LEFT JOIN body bt ON bt.id = mc.troop_id LEFT JOIN body bp ON bp.id = mc.patrol_id
         LEFT JOIN LATERAL (SELECT care_of, line1, line2, postcode, city, country_code FROM person_address
                            WHERE person_id = mc.id ORDER BY is_primary DESC, address_type LIMIT 1) ad ON true
         LEFT JOIN LATERAL (SELECT name, email, mobile_phone, home_phone FROM guardian WHERE child_id = mc.id AND position = 1 LIMIT 1) g1 ON true
         LEFT JOIN LATERAL (SELECT name, email, mobile_phone, home_phone FROM guardian WHERE child_id = mc.id AND position = 2 LIMIT 1) g2 ON true
         LEFT JOIN term_fee fc ON fc.person_id = mc.id AND fc.group_id = mc.group_id AND fc.term_id = $3
         LEFT JOIN term_fee fp ON fp.person_id = mc.id AND fp.group_id = mc.group_id AND fp.term_id = $4
         WHERE mc.group_id = $1 AND mc.membership_status = ANY($2)
         ORDER BY mc.member_no",
    )
    .bind(group_id)
    .bind(&statuses)
    .bind(term.id())
    .bind(term.previous().id())
    .fetch_all(&state.db)
    .await?;

    let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    let roles: Vec<RoleRow> = sqlx::query_as(
        "SELECT ra.person_id, b.kind, b.name, b.number, ra.role_key FROM role_assignment ra JOIN body b ON b.id = ra.body_id
         WHERE ra.person_id = ANY($1) AND ra.valid @> current_date AND b.kind IN ('group', 'troop', 'patrol')",
    )
    .bind(&ids)
    .fetch_all(&state.db)
    .await?;
    let mut roles_by: HashMap<Uuid, Vec<&RoleRow>> = HashMap::new();
    for r in &roles {
        roles_by.entry(r.0).or_default().push(r);
    }

    let mut data = Map::new();
    for r in &rows {
        let mut m = Map::new();
        m.insert("member_no".into(), vr(r.member_no.to_string(), r.member_no));
        m.insert("first_name".into(), vr(r.first_name.clone(), r.first_name.clone()));
        m.insert("last_name".into(), vr(r.last_name.clone(), r.last_name.clone()));
        put(&mut m, "nickname", r.nickname.clone());
        if ssno && !r.protected_identity {
            put(&mut m, "ssno", r.national_id.clone());
        }
        m.insert("date_of_birth".into(), vr(r.birth_date.to_string(), r.birth_date.to_string()));
        let (sex_label, sex_raw) = match r.sex.as_str() {
            "male" => ("Man", "1"),
            "female" => ("Kvinna", "2"),
            "other" => ("Annat", "3"),
            _ => ("Okänt", "0"),
        };
        m.insert("sex".into(), vr(sex_label, sex_raw));
        let status = match r.membership_status.as_str() {
            "active" => "Aktiv",
            "waiting" => "Väntelista",
            "pending" => "Väntar på godkännande",
            other => other,
        };
        m.insert("status".into(), vr(status, r.membership_status.clone()));
        m.insert("created_at".into(), vr(r.created_at.format("%Y-%m-%d").to_string(), r.created_at.to_rfc3339()));
        put(&mut m, "confirmed_at", r.membership_confirmed_at.map(|d| d.to_string()));
        put(&mut m, "waiting_since", r.waiting_since.map(|d| d.to_string()));
        if r.membership_status == "pending" {
            m.insert("awaiting_approval".into(), vr("Ja", 1));
        }
        m.insert("group".into(), vr(r.group_name.clone(), group_no));
        if let (Some(name), Some(no)) = (&r.troop_name, r.troop_number) {
            m.insert("unit".into(), vr(name.clone(), no));
            if let Some(t) = r.troop_type.as_deref().and_then(|t| t.parse::<TroopType>().ok()) {
                m.insert("unit_type".into(), vr(t.label_sv(), t.as_str()));
            }
        }
        if let (Some(name), Some(no)) = (&r.patrol_name, r.patrol_number) {
            m.insert("patrol".into(), vr(name.clone(), no));
        }
        let my_roles = roles_by.get(&r.id).cloned().unwrap_or_default();
        let unit_role =
            my_roles.iter().filter(|x| x.1 == "troop").find_map(|x| unit_role_code(&x.4).map(|c| (c, &x.4)));
        match unit_role {
            Some((code, key)) => {
                m.insert("unit_role".into(), vr(role_label(key), code));
            }
            None if r.is_leader => {
                m.insert("unit_role".into(), vr("Ledare", 3));
            }
            None => {}
        }
        if let Some(g) = my_roles.iter().find(|x| x.1 == "group") {
            m.insert("group_role".into(), vr(role_label(&g.4), g.4.clone()));
        }
        if !my_roles.is_empty() {
            let mut raw = Map::new();
            for (_, kind, _, number, key) in my_roles.iter().copied() {
                let bucket = raw.entry(kind.clone()).or_insert_with(|| Value::Object(Map::new()));
                if let Value::Object(b) = bucket {
                    let inner = b.entry(number.to_string()).or_insert_with(|| Value::Object(Map::new()));
                    if let Value::Object(i) = inner {
                        i.insert(key.clone(), Value::String(role_label(key).to_owned()));
                    }
                }
            }
            let labels: Vec<String> = my_roles.iter().map(|x| format!("{} ({})", role_label(&x.4), x.2)).collect();
            m.insert("roles".into(), vr(labels.join(", "), Value::Object(raw)));
        }
        if contact {
            if !r.protected_identity {
                put(&mut m, "address_co", r.care_of.clone());
                put(&mut m, "address_1", r.line1.clone());
                put(&mut m, "address_2", r.line2.clone());
                put(&mut m, "postcode", r.postcode.clone());
                put(&mut m, "town", r.city.clone());
                put(
                    &mut m,
                    "country",
                    r.country_code.clone().map(|c| if c == "SE" { "Sverige".to_owned() } else { c }),
                );
            }
            put(&mut m, "email", r.email.clone());
            put(&mut m, "contact_alt_email", r.alt_email.clone());
            put(&mut m, "contact_mobile_phone", r.mobile_phone.clone());
            put(&mut m, "contact_home_phone", r.home_phone.clone());
            put(&mut m, "contact_mothers_name", r.g1_name.clone());
            put(&mut m, "contact_email_mum", r.g1_email.clone());
            put(&mut m, "contact_mobile_mum", r.g1_mobile.clone());
            put(&mut m, "contact_telephone_mum", r.g1_home.clone());
            put(&mut m, "contact_fathers_name", r.g2_name.clone());
            put(&mut m, "contact_email_dad", r.g2_email.clone());
            put(&mut m, "contact_mobile_dad", r.g2_mobile.clone());
            put(&mut m, "contact_telephone_dad", r.g2_home.clone());
            if r.can_help {
                m.insert("contact_leader_interest".into(), vr("Ja", 1));
            }
        }
        if r.cur_paid.is_some() {
            m.insert("current_term".into(), vr(term.to_string(), term.id()));
        } else if let Some(due) = r.cur_due {
            m.insert("current_term_due_date".into(), vr(due.to_string(), due.to_string()));
        }
        if r.prev_paid.is_some() {
            m.insert("prev_term".into(), vr(term.previous().to_string(), term.previous().id()));
        }
        put(&mut m, "kid", r.cur_kid.clone());
        data.insert(r.member_no.to_string(), Value::Object(m));
    }

    let labels = json!({
        "member_no": "Medlemsnummer", "first_name": "Förnamn", "last_name": "Efternamn", "nickname": "Smeknamn",
        "ssno": "Personnummer", "date_of_birth": "Födelsedatum", "sex": "Kön", "status": "Status",
        "created_at": "Skapad", "confirmed_at": "Bekräftad", "waiting_since": "På väntelistan sedan",
        "group": "Kår", "unit": "Avdelning", "unit_type": "Avdelningstyp", "unit_role": "Avdelningsroll",
        "group_role": "Kårroll", "patrol": "Patrull", "roles": "Roller", "address_co": "c/o", "address_1": "Adress",
        "address_2": "Adress 2", "postcode": "Postnummer", "town": "Ort", "country": "Land", "email": "E-post",
        "contact_alt_email": "Alternativ e-post", "contact_mobile_phone": "Mobiltelefon", "contact_home_phone": "Hemtelefon",
        "contact_mothers_name": "Anhörig 1", "contact_email_mum": "Anhörig 1 e-post", "contact_mobile_mum": "Anhörig 1 mobil",
        "contact_telephone_mum": "Anhörig 1 hemtelefon", "contact_fathers_name": "Anhörig 2", "contact_email_dad": "Anhörig 2 e-post",
        "contact_mobile_dad": "Anhörig 2 mobil", "contact_telephone_dad": "Anhörig 2 hemtelefon",
        "contact_leader_interest": "Kan tänka sig att hjälpa till", "current_term": "Betald termin",
        "current_term_due_date": "Förfallodag", "prev_term": "Föregående termin", "kid": "OCR/KID"
    });

    audit(
        &state.db,
        None,
        "compat.memberlist",
        "api_key",
        key_id,
        json!({ "group": group_no, "key_name": key_name, "count": rows.len(), "ssno": ssno, "contact": contact, "persons": ids }),
    )
    .await;

    let mut response = Json(json!({ "data": data, "labels": labels })).into_response();
    if via_query {
        // Nyckel i adressen fungerar för att gamla integrationer ska kunna flytta, men ska bort.
        // Headervärden måste vara ASCII.
        response.headers_mut().insert("deprecation", HeaderValue::from_static("true"));
        response.headers_mut().insert(
            "warning",
            HeaderValue::from_static("299 - \"API key in query string is deprecated, use HTTP Basic\""),
        );
    }
    Ok(response)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/bodies/{id}/api-keys", get(list_keys).post(create_key))
        .route("/v1/api-keys/{id}", delete(revoke_key))
        .route("/api/group/memberlist", get(memberlist))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_auth_is_preferred_over_query() {
        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, HeaderValue::from_static("Basic MTAwMTpzZWNyZXQ="));
        let q = MemberlistQuery { id: Some("9".into()), key: Some("x".into()), waiting: None, awaiting_approval: None };
        assert_eq!(credentials(&h, &q), Some(("1001".into(), "secret".into(), false)));
        assert_eq!(credentials(&HeaderMap::new(), &q), Some(("9".into(), "x".into(), true)));
    }
}
