use axum::Router;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::AppState;

pub mod attendance;
pub mod audit_log;
pub mod bodies;
pub mod bulk;
pub mod calendar;
pub mod compat;
pub mod grants;
pub mod me;
pub mod members;
pub mod people;
pub mod views;

/// Kort referens till en enhet. `kind` och `number` bildar den läsbara nyckeln `kind-number`
/// (t.ex. `group-1001`), som kan användas i stället för `id` i alla `/v1/bodies/{id}`-anrop.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BodyRef {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub number: i32,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(me::router())
        .merge(bodies::router())
        .merge(members::router())
        .merge(people::router())
        .merge(attendance::router())
        .merge(views::router())
        .merge(grants::router())
        .merge(calendar::router())
        .merge(bulk::router())
        .merge(audit_log::router())
        .merge(compat::router())
}

/// Registrerar en händelse i revisionsloggen. Fel här får inte stoppa läsningen,
/// men loggas så att de syns i driften. `account_id` saknas för integrationer (API-nycklar).
pub async fn audit(
    db: &sqlx::PgPool,
    account_id: impl Into<Option<Uuid>>,
    action: &str,
    subject_type: &str,
    subject_id: Uuid,
    detail: serde_json::Value,
) {
    if let Err(e) = sqlx::query(
        "INSERT INTO audit_event (account_id, action, subject_type, subject_id, detail) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(account_id.into())
    .bind(action)
    .bind(subject_type)
    .bind(subject_id)
    .bind(detail)
    .execute(db)
    .await
    {
        tracing::error!(error = %e, action, "kunde inte skriva revisionslogg");
    }
}
