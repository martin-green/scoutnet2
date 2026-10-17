use axum::{
    Json,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use utoipa::ToSchema;

/// Fel enligt RFC 9457 (`application/problem+json`).
#[derive(Debug, Serialize, ToSchema)]
pub struct Problem {
    /// Maskinläsbar felkod.
    pub code: &'static str,
    pub title: String,
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("inte inloggad")]
    Unauthenticated,
    #[error("saknar behörighet")]
    Forbidden,
    /// Används även när resursen finns men användaren inte får se den,
    /// så att det inte går att avgöra om den existerar.
    #[error("hittades inte")]
    NotFound,
    #[error("{0}")]
    BadRequest(String),
    /// Resursen är i ett tillstånd som inte tillåter ändringen (t.ex. ett möte som är rapporterat och låst).
    #[error("{0}")]
    Conflict(String),
    #[error("kräver färsk inloggning med MFA")]
    StepUpRequired,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

pub type ApiResult<T> = Result<T, ApiError>;

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            Self::Unauthenticated => (StatusCode::UNAUTHORIZED, "unauthenticated"),
            Self::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            Self::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            Self::Conflict(_) => (StatusCode::CONFLICT, "conflict"),
            Self::StepUpRequired => (StatusCode::UNAUTHORIZED, "step_up_required"),
            Self::Database(_) | Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        };
        let detail = match &self {
            Self::BadRequest(msg) | Self::Conflict(msg) => Some(msg.clone()),
            Self::Database(e) => {
                tracing::error!(error = %e, "databasfel");
                None
            }
            Self::Internal(e) => {
                tracing::error!(error = ?e, "internt fel");
                None
            }
            _ => None,
        };
        let body = Problem { code, title: self.to_string(), status: status.as_u16(), detail };
        (status, [(header::CONTENT_TYPE, "application/problem+json")], Json(body)).into_response()
    }
}
