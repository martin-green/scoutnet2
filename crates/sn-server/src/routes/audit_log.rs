//! "Vem har sett mina uppgifter?" Revisionsloggen för en person, för personen själv, dennes
//! vårdnadshavare och roller med `audit_read` (t.ex. kårordförande).

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState,
    auth::CurrentUser,
    authz::{Permission, role_label},
    error::{ApiError, ApiResult},
};

#[derive(Debug, Serialize, ToSchema)]
pub struct AuditEntry {
    pub at: DateTime<Utc>,
    /// Maskinläsbar händelse, t.ex. `person.read`, `person.national_id.reveal`, `bulk.move`.
    pub action: String,
    /// Händelsen på svenska.
    pub description: String,
    /// Vem som gjorde det. `null` om kontot har tagits bort.
    pub actor: Option<String>,
    /// Aktörens nuvarande roller, för att förstå varför hen hade åtkomst.
    pub actor_roles: Vec<String>,
    /// Om det var personen själv.
    pub by_self: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuditLog {
    pub person_id: Uuid,
    pub entries: Vec<AuditEntry>,
    /// Antal visningar av andra än personen själv de senaste 365 dagarna.
    pub views_by_others: i64,
}

#[derive(sqlx::FromRow)]
struct Row {
    at: DateTime<Utc>,
    action: String,
    detail: serde_json::Value,
    actor_person: Option<Uuid>,
    actor_name: Option<String>,
}

fn describe(action: &str, detail: &serde_json::Value) -> String {
    let fields = detail.get("fields");
    let flag = |k: &str| fields.and_then(|f| f.get(k)).and_then(serde_json::Value::as_bool).unwrap_or(false);
    match action {
        "person.read" => {
            let mut parts = vec!["grunduppgifter"];
            if flag("contact") {
                parts.push("kontaktuppgifter");
            }
            if flag("personal") {
                parts.push("adress och personuppgifter");
            }
            if flag("economy") {
                parts.push("avgifter");
            }
            format!("Öppnade personkortet ({})", parts.join(", "))
        }
        "person.national_id.reveal" => "Visade hela personnumret".into(),
        "bulk.move" => "Flyttade till annan avdelning eller patrull".into(),
        "bulk.admit" => "Tog in från väntelistan".into(),
        "grant.dak.export" => "Exporterade närvaro med personnummer till kommunen (DAK)".into(),
        "compat.memberlist" => {
            let name = detail.get("key_name").and_then(serde_json::Value::as_str).unwrap_or("okänd");
            let mut parts = vec!["grunduppgifter"];
            if detail.get("contact").and_then(serde_json::Value::as_bool) == Some(true) {
                parts.push("kontaktuppgifter");
            }
            if detail.get("ssno").and_then(serde_json::Value::as_bool) == Some(true) {
                parts.push("personnummer");
            }
            format!("Hämtades av integrationen «{name}» ({})", parts.join(", "))
        }
        other => other.to_owned(),
    }
}

/// Händelser som rör en person: visningar av personkortet, visade personnummer och ändringar.
#[utoipa::path(get, path = "/v1/people/{id}/audit", tag = "members",
    params(("id" = Uuid, Path, description = "Personens id")),
    responses((status = 200, body = AuditLog), (status = 404, body = crate::error::Problem)))]
pub async fn person_audit(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<AuditLog>> {
    let p = &user.0;
    let (path, is_child): (Option<String>, bool) = sqlx::query_as(
        "SELECT (SELECT effective_path::text FROM member_current WHERE id = $1),
                EXISTS (SELECT 1 FROM guardian WHERE child_id = $1 AND account_person_id = $2)",
    )
    .bind(id)
    .bind(p.person_id)
    .fetch_one(&state.db)
    .await?;
    let allowed =
        p.person_id == id || is_child || path.as_deref().is_some_and(|path| p.can(Permission::AuditRead, path));
    if !allowed {
        return Err(ApiError::NotFound);
    }

    let rows = sqlx::query_as::<_, Row>(
        "SELECT e.at, e.action, e.detail, a.person_id AS actor_person, ap.first_name || ' ' || ap.last_name AS actor_name
         FROM audit_event e
         LEFT JOIN account a ON a.id = e.account_id LEFT JOIN person ap ON ap.id = a.person_id
         WHERE (e.subject_type = 'person' AND e.subject_id = $1)
            OR (e.action IN ('bulk.move', 'bulk.admit', 'grant.dak.export', 'compat.memberlist')
                AND jsonb_typeof(e.detail->'persons') = 'array' AND e.detail->'persons' ? $1::text)
         ORDER BY e.at DESC LIMIT 200",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    let actors: Vec<Uuid> = rows.iter().filter_map(|r| r.actor_person).collect();
    let roles: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT ra.person_id, ra.role_key, b.name FROM role_assignment ra JOIN body b ON b.id = ra.body_id
         WHERE ra.person_id = ANY($1) AND ra.valid @> current_date ORDER BY nlevel(b.path)",
    )
    .bind(&actors)
    .fetch_all(&state.db)
    .await?;

    let views_by_others: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_event e LEFT JOIN account a ON a.id = e.account_id
         WHERE e.subject_type = 'person' AND e.subject_id = $1 AND e.at > now() - interval '365 days'
           AND a.person_id IS DISTINCT FROM $1",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    let entries = rows
        .into_iter()
        .map(|r| AuditEntry {
            description: describe(&r.action, &r.detail),
            actor_roles: roles
                .iter()
                .filter(|(pid, ..)| Some(*pid) == r.actor_person)
                .map(|(_, key, body)| format!("{}, {body}", role_label(key)))
                .collect(),
            by_self: r.actor_person == Some(id),
            at: r.at,
            action: r.action,
            actor: r.actor_name,
        })
        .collect();
    Ok(Json(AuditLog { person_id: id, entries, views_by_others }))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/people/{id}/audit", get(person_audit))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptions_are_swedish() {
        let d = describe("person.read", &serde_json::json!({ "fields": { "contact": true, "personal": false } }));
        assert_eq!(d, "Öppnade personkortet (grunduppgifter, kontaktuppgifter)");
        assert_eq!(describe("person.national_id.reveal", &serde_json::json!({})), "Visade hela personnumret");
    }
}
