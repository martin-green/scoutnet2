use axum::Router;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::AppState;

pub mod bodies;
pub mod me;
pub mod members;
pub mod people;

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
    Router::new().merge(me::router()).merge(bodies::router()).merge(members::router()).merge(people::router())
}

/// Registrerar en händelse i revisionsloggen. Fel här får inte stoppa läsningen,
/// men loggas så att de syns i driften.
pub async fn audit(
    db: &sqlx::PgPool,
    account_id: Uuid,
    action: &str,
    subject_type: &str,
    subject_id: Uuid,
    detail: serde_json::Value,
) {
    if let Err(e) = sqlx::query(
        "INSERT INTO audit_event (account_id, action, subject_type, subject_id, detail) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(account_id)
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
