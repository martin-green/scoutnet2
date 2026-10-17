use std::time::Instant;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use chrono::{Days, NaiveDate};
use serde::{Deserialize, Serialize};
use sn_domain::{Personnummer, Term, TroopType};
use sqlx::{Postgres, QueryBuilder};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::bodies::load_visible_body;
use crate::{
    AppState,
    auth::CurrentUser,
    authz::Permission,
    error::{ApiError, ApiResult},
};

const MAX_LIMIT: i64 = 500;

/// Filter, sortering och paginering. Samma språk används av UI:t, sparade vyer och export.
#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct MemberQuery {
    /// Fritext: namn, eller medlemsnummer om bara siffror.
    pub q: Option<String>,
    /// Kommaseparerad lista: `active`, `waiting`, `pending`. Standard `active`.
    pub status: Option<String>,
    /// Avdelning (id).
    pub troop_id: Option<Uuid>,
    /// Kommaseparerad lista av avdelningstyper, t.ex. `sparare,upptackare`.
    pub troop_type: Option<String>,
    /// Födda tidigast detta år.
    pub born_from: Option<i32>,
    /// Födda senast detta år.
    pub born_to: Option<i32>,
    /// `true` = bara ledare, `false` = bara scouter.
    pub leaders: Option<bool>,
    /// Kommaseparerad lista: `male`, `female`, `other`, `unknown`.
    pub sex: Option<String>,
    /// Har vårdnadshavare men ingen av dem har e-post.
    pub missing_guardian_email: Option<bool>,
    /// Status för registerutdrag, kommaseparerad: `missing`, `expired`, `expiring`, `valid`, `not_required`.
    pub police_check: Option<String>,
    /// Avgift innevarande termin, kommaseparerad: `paid`, `unpaid`, `overdue`.
    pub fee_status: Option<String>,
    /// Kommaseparerade sorteringsnycklar, `-` för fallande. T.ex. `troop,-birth_date`.
    /// Nycklar: name, `first_name`, `last_name`, `member_no`, `birth_date`, troop, patrol, `member_since`, `waiting_since`, attendance, `police_check`, fee.
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RefDto {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TroopRefDto {
    pub id: Uuid,
    pub name: String,
    pub troop_type: Option<String>,
    pub troop_type_label: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MemberRow {
    pub id: Uuid,
    pub member_no: i32,
    pub first_name: String,
    pub last_name: String,
    pub birth_date: NaiveDate,
    pub age: i32,
    pub sex: String,
    pub status: String,
    pub member_since: Option<NaiveDate>,
    pub waiting_since: Option<NaiveDate>,
    pub wished_troop_type: Option<String>,
    pub group: RefDto,
    pub troop: Option<TroopRefDto>,
    pub patrol: Option<RefDto>,
    pub is_leader: bool,
    /// Personens status: `active`, `new`, `unregistered`, `auto_unregistered`, `deceased`, `active_non_member`, `anonymized`, `locked`.
    pub person_status: String,
    /// Utelämnas utan behörigheten `members_contact_read` (gäller alla kontaktfält).
    pub email: Option<String>,
    pub alt_email: Option<String>,
    pub mobile_phone: Option<String>,
    pub home_phone: Option<String>,
    /// `ÅÅÅÅMMDD-****`, utelämnas utan `personal_read`. Hela numret kräver ett separat, loggat anrop.
    pub national_id_masked: Option<String>,
    pub guardian_count: i64,
    pub guardians_missing_email: bool,
    /// Andel närvaro innevarande termin (0–1), om möten har hållits.
    pub attendance_pct: Option<f64>,
    /// Registerutdrag: `not_required`, `missing`, `expired`, `expiring` (inom 90 dagar), `valid`. Kräver `personal_read`.
    pub police_check: Option<String>,
    pub police_check_valid_until: Option<NaiveDate>,
    /// Avgiftsgrupp. Kräver `economy_read`.
    pub fee_group: Option<String>,
    /// Terminsavgift innevarande termin: `paid`, `unpaid`, `overdue`. Kräver `economy_read`.
    pub fee_status: Option<String>,
    pub fee_due_date: Option<NaiveDate>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MemberPage {
    pub items: Vec<MemberRow>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
    pub term: String,
    /// Fält som har dolts på minst en rad på grund av behörighet.
    pub redacted_fields: Vec<String>,
    /// Serverns tid för frågan i millisekunder.
    pub took_ms: f64,
}

#[derive(sqlx::FromRow)]
struct Row {
    id: Uuid,
    member_no: i32,
    first_name: String,
    last_name: String,
    birth_date: NaiveDate,
    age: i32,
    sex: String,
    membership_status: String,
    member_since: Option<NaiveDate>,
    waiting_since: Option<NaiveDate>,
    wished_troop_type: Option<String>,
    group_id: Uuid,
    group_name: String,
    troop_id: Option<Uuid>,
    troop_name: Option<String>,
    troop_type: Option<String>,
    patrol_id: Option<Uuid>,
    patrol_name: Option<String>,
    is_leader: bool,
    person_status: String,
    email: Option<String>,
    alt_email: Option<String>,
    mobile_phone: Option<String>,
    home_phone: Option<String>,
    national_id: Option<String>,
    effective_path: String,
    guardian_count: i64,
    guardian_email_count: i64,
    attendance_pct: Option<f64>,
    police_check: Option<String>,
    police_check_valid_until: Option<NaiveDate>,
    fee_group: Option<String>,
    fee_status: Option<String>,
    fee_due_date: Option<NaiveDate>,
    total: i64,
}

pub(super) fn csv_list(s: Option<&str>) -> Vec<String> {
    s.map(|s| s.split(',').map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned).collect()).unwrap_or_default()
}

pub(super) fn validate_in(values: &[String], allowed: &[&str], field: &str) -> ApiResult<()> {
    if let Some(bad) = values.iter().find(|v| !allowed.contains(&v.as_str())) {
        return Err(ApiError::BadRequest(format!("ogiltigt värde för {field}: {bad}")));
    }
    Ok(())
}

/// Översätter sorteringsnycklar till SQL. Endast vitlistade uttryck används.
fn order_by(sort: Option<&str>) -> ApiResult<String> {
    let mut parts = Vec::new();
    for key in csv_list(sort.or(Some("name")).filter(|s| !s.trim().is_empty())) {
        let (desc, key) = key.strip_prefix('-').map_or((false, key.as_str()), |k| (true, k));
        let dir = if desc { "DESC" } else { "ASC" };
        let rev = if desc { "ASC" } else { "DESC" };
        let expr = match key {
            "name" => format!("mc.last_name {dir}, mc.first_name {dir}"),
            "first_name" => format!("mc.first_name {dir}"),
            "last_name" => format!("mc.last_name {dir}"),
            "member_no" => format!("mc.member_no {dir}"),
            "birth_date" => format!("mc.birth_date {dir}"),
            "age" => format!("mc.birth_date {rev}"),
            "troop" => format!("mc.troop_name {dir} NULLS LAST"),
            "patrol" => format!("mc.patrol_name {dir} NULLS LAST"),
            "member_since" => format!("mc.member_since {dir}"),
            "waiting_since" => format!("mc.waiting_since {dir} NULLS LAST"),
            "attendance" => format!("att.pct {dir} NULLS LAST"),
            "police_check" => format!(
                "array_position(ARRAY['missing','expired','expiring','valid','not_required'], pc.status) {dir} NULLS LAST"
            ),
            "fee" => format!("array_position(ARRAY['overdue','unpaid','paid'], fee.status) {dir} NULLS LAST"),
            other => return Err(ApiError::BadRequest(format!("okänd sorteringsnyckel: {other}"))),
        };
        parts.push(expr);
    }
    parts.push("mc.last_name, mc.first_name, mc.id".into());
    Ok(parts.join(", "))
}

/// Medlemmar i en enhet (inklusive underliggande enheter) som den inloggade får se.
#[utoipa::path(get, path = "/v1/bodies/{id}/members", tag = "members",
    params(("id" = String, Path, description = "Enhetens id (UUID) eller nyckel `typ-nummer`, t.ex. `group-1001`"), MemberQuery),
    responses((status = 200, body = MemberPage), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
#[expect(clippy::too_many_lines, reason = "filterbyggaren är linjär och tydligast i ett stycke")]
pub async fn list_members(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(key): Path<String>,
    Query(q): Query<MemberQuery>,
) -> ApiResult<Json<MemberPage>> {
    let started = Instant::now();
    let (_, _, body_path) = load_visible_body(&state, &user, &key, Permission::MembersRead).await?;
    let roots = user.0.roots_for(Permission::MembersRead);
    let personal_roots = user.0.roots_for(Permission::PersonalRead);
    let economy_roots = user.0.roots_for(Permission::EconomyRead);

    let statuses = {
        let s = csv_list(q.status.as_deref());
        if s.is_empty() { vec!["active".to_owned()] } else { s }
    };
    validate_in(&statuses, &["active", "waiting", "pending"], "status")?;
    let troop_types = csv_list(q.troop_type.as_deref());
    validate_in(&troop_types, &TroopType::ALL.map(TroopType::as_str), "troop_type")?;
    let sexes = csv_list(q.sex.as_deref());
    validate_in(&sexes, &["male", "female", "other", "unknown"], "sex")?;
    let police = csv_list(q.police_check.as_deref());
    validate_in(&police, &["missing", "expired", "expiring", "valid", "not_required"], "police_check")?;
    let fee_statuses = csv_list(q.fee_status.as_deref());
    validate_in(&fee_statuses, &["paid", "unpaid", "overdue"], "fee_status")?;
    let order = order_by(q.sort.as_deref())?;
    let limit = q.limit.unwrap_or(100).clamp(1, MAX_LIMIT);
    let offset = q.offset.unwrap_or(0).max(0);

    let term = Term::containing(chrono::Local::now().date_naive());
    let term_end = term.end().checked_add_days(Days::new(1)).unwrap_or_else(|| term.end());

    // Registerutdrag och avgifter räknas bara fram där användaren har behörighet, så att filter
    // och sortering inte kan användas för att härleda dolda uppgifter.
    let mut qb = QueryBuilder::<Postgres>::new(
        "SELECT mc.id, mc.member_no, mc.first_name, mc.last_name, mc.birth_date,
                date_part('year', age(mc.birth_date))::int AS age, mc.sex, mc.membership_status, mc.member_since,
                mc.waiting_since, mc.wished_troop_type, mc.group_id, mc.group_name, mc.troop_id, mc.troop_name,
                mc.troop_type, mc.patrol_id, mc.patrol_name, mc.is_leader, mc.person_status,
                mc.email, mc.alt_email, mc.mobile_phone, mc.home_phone, mc.national_id,
                mc.effective_path::text AS effective_path,
                gu.n AS guardian_count, gu.n_email AS guardian_email_count, att.pct AS attendance_pct,
                pc.status AS police_check, pc.valid_until AS police_check_valid_until,
                fee.fee_group, fee.status AS fee_status, fee.due_date AS fee_due_date,
                count(*) OVER () AS total
         FROM member_current mc
         LEFT JOIN LATERAL (SELECT count(*) AS n, count(g.email) AS n_email FROM guardian g WHERE g.child_id = mc.id) gu ON true
         LEFT JOIN LATERAL (
             SELECT (count(*) FILTER (WHERE a.status = 'present'))::float8 / nullif(count(*), 0) AS pct
             FROM attendance a JOIN meeting mt ON mt.id = a.meeting_id
             WHERE a.person_id = mc.id AND mt.starts_at < now() AND mt.starts_at >= ",
    );
    qb.push_bind(term.start()).push(" AND mt.starts_at < ").push_bind(term_end).push(") att ON true");
    qb.push(
        " LEFT JOIN LATERAL (
             SELECT CASE WHEN NOT s.required THEN 'not_required'
                         WHEN s.shown_on IS NULL THEN 'missing'
                         WHEN s.valid_until < current_date THEN 'expired'
                         WHEN s.valid_until < current_date + 90 THEN 'expiring'
                         ELSE 'valid' END AS status,
                    s.valid_until
             FROM police_check_status s
             WHERE s.person_id = mc.id AND mc.effective_path <@ ANY(",
    );
    qb.push_bind(&personal_roots).push("::text[]::ltree[])) pc ON true");
    qb.push(
        " LEFT JOIN LATERAL (
             SELECT mc.fee_group,
                    CASE WHEN tf.paid_on IS NOT NULL THEN 'paid'
                         WHEN tf.due_date < current_date THEN 'overdue'
                         WHEN tf.id IS NOT NULL THEN 'unpaid' END AS status,
                    tf.due_date
             FROM (SELECT 1) one
             LEFT JOIN term_fee tf ON tf.person_id = mc.id AND tf.group_id = mc.group_id AND tf.term_id = ",
    );
    qb.push_bind(term.id());
    qb.push(" WHERE mc.effective_path <@ ANY(").push_bind(&economy_roots).push("::text[]::ltree[])) fee ON true");
    qb.push(" WHERE mc.effective_path <@ ").push_bind(&body_path).push("::ltree");
    qb.push(" AND mc.effective_path <@ ANY(").push_bind(&roots).push("::text[]::ltree[])");
    qb.push(" AND mc.membership_status = ANY(").push_bind(&statuses).push(")");

    if let Some(text) = q.q.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        if text.chars().all(|c| c.is_ascii_digit()) {
            qb.push(" AND mc.member_no::text LIKE ").push_bind(format!("{text}%"));
        } else {
            qb.push(" AND lower(mc.first_name || ' ' || mc.last_name) LIKE ")
                .push_bind(format!("%{}%", text.to_lowercase()));
        }
    }
    if let Some(troop_id) = q.troop_id {
        qb.push(" AND mc.troop_id = ").push_bind(troop_id);
    }
    if !troop_types.is_empty() {
        qb.push(" AND mc.troop_type = ANY(").push_bind(&troop_types).push(")");
    }
    if let Some(from) = q.born_from {
        qb.push(" AND mc.birth_date >= make_date(").push_bind(from).push(", 1, 1)");
    }
    if let Some(to) = q.born_to {
        qb.push(" AND mc.birth_date < make_date(").push_bind(to).push(" + 1, 1, 1)");
    }
    if let Some(leaders) = q.leaders {
        qb.push(" AND mc.is_leader = ").push_bind(leaders);
    }
    if !sexes.is_empty() {
        qb.push(" AND mc.sex = ANY(").push_bind(&sexes).push(")");
    }
    if q.missing_guardian_email == Some(true) {
        qb.push(" AND gu.n > 0 AND gu.n_email = 0");
    }
    if !police.is_empty() {
        qb.push(" AND pc.status = ANY(").push_bind(&police).push(")");
    }
    if !fee_statuses.is_empty() {
        qb.push(" AND fee.status = ANY(").push_bind(&fee_statuses).push(")");
    }
    qb.push(" ORDER BY ").push(order);
    qb.push(" LIMIT ").push_bind(limit).push(" OFFSET ").push_bind(offset);

    let rows: Vec<Row> = qb.build_query_as().fetch_all(&state.db).await?;
    let total = rows.first().map_or(0, |r| r.total);

    let mut redacted = std::collections::BTreeSet::new();
    let items =
        rows.into_iter()
            .map(|r| {
                let contact = user.0.can(Permission::MembersContactRead, &r.effective_path);
                let personal = user.0.can(Permission::PersonalRead, &r.effective_path);
                let economy = user.0.can(Permission::EconomyRead, &r.effective_path);
                if !contact {
                    redacted.extend(["email", "alt_email", "mobile_phone", "home_phone"]);
                }
                if !personal {
                    redacted.extend(["national_id_masked", "police_check"]);
                }
                if !economy {
                    redacted.extend(["fee_group", "fee_status"]);
                }
                MemberRow {
                    id: r.id,
                    member_no: r.member_no,
                    first_name: r.first_name,
                    last_name: r.last_name,
                    birth_date: r.birth_date,
                    age: r.age,
                    sex: r.sex,
                    status: r.membership_status,
                    member_since: r.member_since,
                    waiting_since: r.waiting_since,
                    wished_troop_type: r.wished_troop_type,
                    group: RefDto { id: r.group_id, name: r.group_name },
                    troop: r.troop_id.zip(r.troop_name).map(|(id, name)| TroopRefDto {
                        id,
                        name,
                        troop_type_label: r
                            .troop_type
                            .as_deref()
                            .and_then(|t| t.parse::<TroopType>().ok())
                            .map(|t| t.label_sv().into()),
                        troop_type: r.troop_type,
                    }),
                    patrol: r.patrol_id.zip(r.patrol_name).map(|(id, name)| RefDto { id, name }),
                    is_leader: r.is_leader,
                    person_status: r.person_status,
                    email: r.email.filter(|_| contact),
                    alt_email: r.alt_email.filter(|_| contact),
                    mobile_phone: r.mobile_phone.filter(|_| contact),
                    home_phone: r.home_phone.filter(|_| contact),
                    national_id_masked: r.national_id.filter(|_| personal).map(|n| {
                        n.parse::<Personnummer>().map_or_else(|_| "********-****".into(), Personnummer::masked)
                    }),
                    guardian_count: r.guardian_count,
                    guardians_missing_email: r.guardian_count > 0 && r.guardian_email_count == 0,
                    attendance_pct: r.attendance_pct,
                    police_check: r.police_check.filter(|_| personal),
                    police_check_valid_until: r.police_check_valid_until.filter(|_| personal),
                    fee_group: r.fee_group.filter(|_| economy),
                    fee_status: r.fee_status.filter(|_| economy),
                    fee_due_date: r.fee_due_date.filter(|_| economy),
                }
            })
            .collect();

    Ok(Json(MemberPage {
        items,
        total,
        limit,
        offset,
        term: term.to_string(),
        redacted_fields: redacted.into_iter().map(str::to_owned).collect(),
        took_ms: started.elapsed().as_secs_f64() * 1000.0,
    }))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/bodies/{id}/members", get(list_members))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_by_whitelists_keys() {
        assert!(
            order_by(Some("troop,-birth_date"))
                .is_ok_and(|s| s.starts_with("mc.troop_name ASC NULLS LAST, mc.birth_date DESC"))
        );
        assert!(order_by(Some("name; DROP TABLE person")).is_err());
        assert!(order_by(None).is_ok_and(|s| s.starts_with("mc.last_name ASC")));
    }
}
