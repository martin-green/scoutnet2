//! Sessioner (BFF-mönster) och snabbinloggning med personas för utveckling och demo.
//!
//! Produktion: inloggning sker uteslutande via `ScoutID` (OIDC). Sessionen är densamma,
//! bara sättet att skapa den skiljer sig (se planen 7.2).

use axum::{
    Json, Router,
    extract::{FromRequestParts, State},
    http::{StatusCode, request::Parts},
    response::IntoResponse,
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState,
    authz::{Grant, Principal},
    error::{ApiError, ApiResult},
};

/// Extraktor för inloggad användare. Ger 401 om sessionen saknas eller har gått ut.
#[derive(Debug, Clone)]
pub struct CurrentUser(pub Principal);

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar
            .get(state.config.session_cookie_name())
            .map(|c| c.value().to_owned())
            .ok_or(ApiError::Unauthenticated)?;
        load_principal(state, &token).await?.map(CurrentUser).ok_or(ApiError::Unauthenticated)
    }
}

pub(crate) fn hash_token(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

#[derive(sqlx::FromRow)]
struct SessionRow {
    account_id: Uuid,
    person_id: Uuid,
    member_no: i32,
    display_name: String,
    auth_level: String,
    auth_time: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct GrantRow {
    body_id: Uuid,
    body_path: String,
    body_name: String,
    body_kind: String,
    body_number: i32,
    role_key: String,
}

/// Personens aktuella rolltilldelningar. Används för sessioner och för kalenderflöden.
pub(crate) async fn load_grants(db: &sqlx::PgPool, person_id: Uuid) -> ApiResult<Vec<Grant>> {
    Ok(sqlx::query_as::<_, GrantRow>(
        "SELECT b.id AS body_id, b.path::text AS body_path, b.name AS body_name, b.kind AS body_kind,
                b.number AS body_number, ra.role_key
         FROM role_assignment ra JOIN body b ON b.id = ra.body_id
         WHERE ra.person_id = $1 AND ra.valid @> current_date
         ORDER BY nlevel(b.path), b.name",
    )
    .bind(person_id)
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|g| Grant {
        body_id: g.body_id,
        body_path: g.body_path,
        body_name: g.body_name,
        body_kind: g.body_kind,
        body_number: g.body_number,
        role_key: g.role_key,
    })
    .collect())
}

async fn load_principal(state: &AppState, token: &str) -> ApiResult<Option<Principal>> {
    let Some(row) = sqlx::query_as::<_, SessionRow>(
        "SELECT a.id AS account_id, p.id AS person_id, p.member_no, p.first_name || ' ' || p.last_name AS display_name,
                s.auth_level, s.auth_time
         FROM session s JOIN account a ON a.id = s.account_id JOIN person p ON p.id = a.person_id
         WHERE s.token_hash = $1 AND s.expires_at > now()",
    )
    .bind(hash_token(token))
    .fetch_optional(&state.db)
    .await?
    else {
        return Ok(None);
    };

    let grants = load_grants(&state.db, row.person_id).await?;

    Ok(Some(Principal {
        account_id: row.account_id,
        person_id: row.person_id,
        member_no: row.member_no,
        display_name: row.display_name,
        auth_level: row.auth_level,
        auth_time: row.auth_time,
        grants,
    }))
}

/// Skapar en session och returnerar den slumpmässiga token som ska ligga i cookien.
/// `auth_time` kommer från ScoutID (när användaren faktiskt autentiserade sig) och styr step-up.
pub async fn create_session(
    state: &AppState,
    account_id: Uuid,
    auth_level: &str,
    auth_time: Option<DateTime<Utc>>,
    id_token: Option<&str>,
) -> ApiResult<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("slumptal: {e}"))?;
    let token = hex::encode(bytes);
    sqlx::query(
        "INSERT INTO session (token_hash, account_id, auth_level, auth_time, id_token, expires_at)
         VALUES ($1, $2, $3, coalesce($4, now()), $5, now() + make_interval(hours => $6))",
    )
    .bind(hash_token(&token))
    .bind(account_id)
    .bind(auth_level)
    .bind(auth_time)
    .bind(id_token)
    .bind(i32::try_from(state.config.session_hours).unwrap_or(12))
    .execute(&state.db)
    .await?;
    sqlx::query("UPDATE account SET last_login_at = now() WHERE id = $1").bind(account_id).execute(&state.db).await?;
    Ok(token)
}

pub(crate) fn session_cookie(state: &AppState, token: String) -> Cookie<'static> {
    Cookie::build((state.config.session_cookie_name(), token))
        .http_only(true)
        .secure(state.config.cookie_secure)
        .same_site(SameSite::Lax)
        .path("/")
        .build()
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Persona {
    pub account_id: Uuid,
    pub persona: String,
    pub name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DevLoginRequest {
    pub account_id: Uuid,
}

/// Lista personas för snabbinloggning (endast när `SN_DEV_LOGIN` är på).
#[utoipa::path(get, path = "/auth/dev/personas", tag = "auth", responses((status = 200, body = [Persona])))]
pub async fn dev_personas(State(state): State<AppState>) -> ApiResult<Json<Vec<Persona>>> {
    if !state.config.dev_login {
        return Err(ApiError::NotFound);
    }
    let rows = sqlx::query_as::<_, (Uuid, String, String)>(
        "SELECT a.id, a.dev_persona, p.first_name || ' ' || p.last_name
         FROM account a JOIN person p ON p.id = a.person_id
         WHERE a.dev_persona IS NOT NULL ORDER BY a.id",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows.into_iter().map(|(account_id, persona, name)| Persona { account_id, persona, name }).collect()))
}

/// Logga in som en persona (endast utveckling och demo).
#[utoipa::path(post, path = "/auth/dev/login", tag = "auth", request_body = DevLoginRequest, responses((status = 204)))]
pub async fn dev_login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(req): Json<DevLoginRequest>,
) -> ApiResult<impl IntoResponse> {
    if !state.config.dev_login {
        return Err(ApiError::NotFound);
    }
    let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM account WHERE id = $1 AND dev_persona IS NOT NULL")
        .bind(req.account_id)
        .fetch_optional(&state.db)
        .await?;
    if exists.is_none() {
        return Err(ApiError::NotFound);
    }
    let token = create_session(&state, req.account_id, "dev", None, None).await?;
    Ok((jar.add(session_cookie(&state, token)), StatusCode::NO_CONTENT))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LogoutResponse {
    /// Om sessionen kom från ScoutID: adress för att logga ut även där (RP-initierad utloggning).
    pub logout_url: Option<String>,
}

/// Logga ut: tar bort sessionen på servern och cookien i webbläsaren. Ger `logout_url` om
/// användaren även ska loggas ut från ScoutID.
#[utoipa::path(post, path = "/auth/logout", tag = "auth", responses((status = 200, body = LogoutResponse)))]
pub async fn logout(State(state): State<AppState>, jar: CookieJar) -> ApiResult<impl IntoResponse> {
    let name = state.config.session_cookie_name();
    let mut logout_url = None;
    if let Some(c) = jar.get(name) {
        let id_token: Option<(Option<String>,)> =
            sqlx::query_as("DELETE FROM session WHERE token_hash = $1 RETURNING id_token")
                .bind(hash_token(c.value()))
                .fetch_optional(&state.db)
                .await?;
        if let (Some((Some(id_token),)), Some(oidc)) = (id_token, state.oidc.as_ref()) {
            logout_url = oidc.logout_url(&id_token).await;
        }
    }
    Ok((jar.remove(Cookie::build(name).path("/")), Json(LogoutResponse { logout_url })))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/dev/personas", get(dev_personas))
        .route("/auth/dev/login", post(dev_login))
        .route("/auth/logout", post(logout))
}
