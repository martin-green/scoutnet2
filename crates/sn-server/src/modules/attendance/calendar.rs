//! Kalenderprenumeration (iCal, RFC 5545) för en enhets möten. Adressen innehåller en slumpmässig
//! token som kan återkallas. Flödet innehåller inga personuppgifter, och behörigheten kontrolleras
//! vid varje hämtning, så en borttagen roll stänger flödet direkt.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::routes::bodies::load_visible_body;
use crate::{
    AppState,
    auth::{CurrentUser, hash_token, load_grants},
    authz::{Permission, Principal},
    error::{ApiError, ApiResult},
};

#[derive(Debug, Serialize, ToSchema)]
pub struct CalendarSubscription {
    /// Prenumerationsadress (webcal/https). Visas bara när den skapas.
    pub url: String,
}

/// Skapar en ny kalenderadress för enheten. En tidigare adress för samma enhet slutar fungera.
#[utoipa::path(post, path = "/v1/bodies/{id}/calendar", tag = "attendance",
    params(("id" = String, Path, description = "Enhetens id eller nyckel, t.ex. `troop-5003`")),
    responses((status = 201, body = CalendarSubscription), (status = 404, body = crate::error::Problem)))]
pub async fn create_calendar(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(key): Path<String>,
) -> ApiResult<(StatusCode, Json<CalendarSubscription>)> {
    let (body_id, ..) = load_visible_body(&state, &user, &key, Permission::AttendanceRead).await?;
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("slumptal: {e}"))?;
    let token = hex::encode(bytes);
    sqlx::query(
        "INSERT INTO calendar_token (token_hash, account_id, body_id) VALUES ($1, $2, $3)
         ON CONFLICT (account_id, body_id) DO UPDATE SET token_hash = EXCLUDED.token_hash, created_at = now(), last_used_at = NULL",
    )
    .bind(hash_token(&token))
    .bind(user.0.account_id)
    .bind(body_id)
    .execute(&state.db)
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(CalendarSubscription { url: format!("{}/ical/{token}.ics", state.config.public_url) }),
    ))
}

/// Återkallar kalenderadressen för enheten.
#[utoipa::path(delete, path = "/v1/bodies/{id}/calendar", tag = "attendance",
    params(("id" = String, Path, description = "Enhetens id eller nyckel")),
    responses((status = 204), (status = 404, body = crate::error::Problem)))]
pub async fn delete_calendar(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(key): Path<String>,
) -> ApiResult<StatusCode> {
    let (body_id, ..) = load_visible_body(&state, &user, &key, Permission::AttendanceRead).await?;
    sqlx::query("DELETE FROM calendar_token WHERE account_id = $1 AND body_id = $2")
        .bind(user.0.account_id)
        .bind(body_id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Text enligt RFC 5545: escape av specialtecken.
fn ics_text(s: &str) -> String {
    s.replace('\\', "\\\\").replace(';', "\\;").replace(',', "\\,").replace(['\r', '\n'], " ")
}

/// Radvikning efter 75 oktetter (RFC 5545 3.1), utan att dela UTF-8-tecken.
fn fold(line: &str) -> String {
    let mut out = String::with_capacity(line.len() + 8);
    let mut width = 0;
    for c in line.chars() {
        if width + c.len_utf8() > 75 {
            out.push_str("\r\n ");
            width = 1;
        }
        out.push(c);
        width += c.len_utf8();
    }
    out.push_str("\r\n");
    out
}

#[derive(sqlx::FromRow)]
struct FeedMeeting {
    id: Uuid,
    troop_name: String,
    name: String,
    kind: String,
    starts_at: DateTime<Utc>,
    duration_min: i32,
    location: Option<String>,
}

/// Kalenderflödet. Ogiltig eller återkallad token, eller förlorad behörighet, ger 404.
pub async fn ical_feed(State(state): State<AppState>, Path(file): Path<String>) -> ApiResult<Response> {
    let token = file.strip_suffix(".ics").ok_or(ApiError::NotFound)?;
    if token.len() != 64 || !token.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ApiError::NotFound);
    }
    let row: Option<(Uuid, Uuid, String, String)> = sqlx::query_as(
        "UPDATE calendar_token ct SET last_used_at = now() FROM account a, body b
         WHERE ct.token_hash = $1 AND a.id = ct.account_id AND b.id = ct.body_id
         RETURNING a.id, a.person_id, b.name, b.path::text",
    )
    .bind(hash_token(token))
    .fetch_optional(&state.db)
    .await?;
    let Some((account_id, person_id, body_name, path)) = row else { return Err(ApiError::NotFound) };
    let principal = Principal {
        account_id,
        person_id,
        member_no: 0,
        display_name: String::new(),
        auth_level: "calendar".into(),
        auth_time: Utc::now(),
        grants: load_grants(&state.db, person_id).await?,
    };
    if !principal.can(Permission::AttendanceRead, &path) || !crate::modules::is_active(&state, super::ID, &path).await?
    {
        return Err(ApiError::NotFound);
    }
    let meetings = sqlx::query_as::<_, FeedMeeting>(
        "SELECT m.id, b.name AS troop_name, m.name, m.kind, m.starts_at, m.duration_min, m.location
         FROM meeting m JOIN body b ON b.id = m.troop_id
         WHERE b.path <@ $1::ltree AND m.starts_at BETWEEN now() - interval '60 days' AND now() + interval '365 days'
         ORDER BY m.starts_at LIMIT 2000",
    )
    .bind(&path)
    .fetch_all(&state.db)
    .await?;

    let fmt = |t: DateTime<Utc>| t.format("%Y%m%dT%H%M%SZ").to_string();
    let now = fmt(Utc::now());
    let mut ics = String::new();
    for line in [
        "BEGIN:VCALENDAR".to_owned(),
        "VERSION:2.0".into(),
        "PRODID:-//Scouterna//Scoutnet 2//SV".into(),
        "CALSCALE:GREGORIAN".into(),
        "METHOD:PUBLISH".into(),
        format!("X-WR-CALNAME:{}", ics_text(&format!("{body_name} – Scoutnet 2"))),
        "X-WR-TIMEZONE:Europe/Stockholm".into(),
        "REFRESH-INTERVAL;VALUE=DURATION:PT6H".into(),
    ] {
        ics.push_str(&fold(&line));
    }
    for m in meetings {
        let end = m.starts_at + chrono::Duration::minutes(i64::from(m.duration_min));
        let mut lines = vec![
            "BEGIN:VEVENT".to_owned(),
            format!("UID:{}@scoutnet2", m.id),
            format!("DTSTAMP:{now}"),
            format!("DTSTART:{}", fmt(m.starts_at)),
            format!("DTEND:{}", fmt(end)),
            format!("SUMMARY:{}", ics_text(&format!("{}: {}", m.troop_name, m.name))),
        ];
        if let Some(loc) = m.location.as_deref() {
            lines.push(format!("LOCATION:{}", ics_text(loc)));
        }
        if m.kind != "meeting" {
            lines.push(format!("CATEGORIES:{}", ics_text(&m.kind)));
        }
        lines.push("END:VEVENT".into());
        for line in lines {
            ics.push_str(&fold(&line));
        }
    }
    ics.push_str("END:VCALENDAR\r\n");
    Ok(([(header::CONTENT_TYPE, "text/calendar; charset=utf-8")], ics).into_response())
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/bodies/{id}/calendar", post(create_calendar).delete(delete_calendar))
        .route("/ical/{file}", get(ical_feed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ics_text_is_escaped_and_folded() {
        assert_eq!(ics_text("Möte; hajk, del 1\nkväll"), "Möte\\; hajk\\, del 1 kväll");
        let long = format!("SUMMARY:{}", "å".repeat(60));
        let folded = fold(&long);
        assert!(folded.split("\r\n").all(|l| l.len() <= 75));
        assert_eq!(folded.replace("\r\n ", "").trim_end(), long);
    }
}
