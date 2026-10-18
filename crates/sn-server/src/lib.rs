//! Scoutnet 2 – API-server (prototyp).

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::Request,
    http::{HeaderName, HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use sqlx::{PgPool, migrate::Migrator};
use tower_http::{
    catch_panic::CatchPanicLayer,
    compression::CompressionLayer,
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};
use utoipa_scalar::{Scalar, Servable};

pub mod auth;
pub mod authz;
pub mod config;
pub mod error;
pub mod modules;
pub mod oidc;
pub mod openapi;
pub mod routes;
pub mod seed;

pub use config::Config;

pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    /// Inloggning via ScoutID, om konfigurerad.
    pub oidc: Option<Arc<oidc::Oidc>>,
}

impl AppState {
    pub fn new(db: PgPool, config: Config) -> anyhow::Result<Self> {
        let oidc = config.oidc.clone().map(|c| oidc::Oidc::new(c, &config.public_url)).transpose()?.map(Arc::new);
        Ok(Self { db, config: Arc::new(config), oidc })
    }
}

async fn healthz(axum::extract::State(state): axum::extract::State<AppState>) -> impl IntoResponse {
    match sqlx::query("SELECT 1").execute(&state.db).await {
        Ok(_) => (StatusCode::OK, "ok"),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "db unavailable"),
    }
}

async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(openapi::spec())
}

/// Muterande anrop kräver en egen header. Webbläsare kan inte skicka den
/// cross-site utan CORS-preflight, vilket stoppar CSRF tillsammans med SameSite-cookien.
async fn csrf_guard(req: Request, next: Next) -> Response {
    let safe = matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    if !safe && req.headers().get("x-sn-csrf").is_none_or(|v| v != "1") {
        return error::ApiError::Forbidden.into_response();
    }
    next.run(req).await
}

fn security_headers(router: Router) -> Router {
    let headers: [(HeaderName, &'static str); 5] = [
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (header::X_FRAME_OPTIONS, "DENY"),
        (header::REFERRER_POLICY, "strict-origin-when-cross-origin"),
        (HeaderName::from_static("permissions-policy"), "camera=(), microphone=(), geolocation=(), payment=()"),
        (HeaderName::from_static("cross-origin-opener-policy"), "same-origin"),
    ];
    headers.into_iter().fold(router, |r, (name, value)| {
        r.layer(SetResponseHeaderLayer::if_not_present(name, HeaderValue::from_static(value)))
    })
}

/// Bygger hela applikationen. Används av `main` och av integrationstesterna.
pub fn app(state: &AppState) -> Router {
    let api = Router::new()
        .merge(routes::router())
        .merge(modules::router(state))
        .merge(auth::router())
        .merge(oidc::router())
        .layer(middleware::from_fn(csrf_guard))
        .layer(SetResponseHeaderLayer::overriding(header::CACHE_CONTROL, HeaderValue::from_static("no-store")))
        .route("/healthz", get(healthz))
        .route("/openapi.json", get(openapi_json))
        .with_state(state.clone());

    let mut app = Router::new().merge(api).merge(Scalar::with_url("/docs", openapi::spec()));

    if let Some(dist) = state.config.web_dist.as_ref() {
        let spa = ServeDir::new(dist).fallback(ServeFile::new(dist.join("200.html")));
        app = app.fallback_service(spa);
    }

    security_headers(app)
        .layer(CompressionLayer::new())
        // En panik i en hanterare ger 500 i stället för en stängd anslutning (och loggas av TraceLayer).
        .layer(CatchPanicLayer::custom(|_| {
            error::ApiError::Internal(anyhow::anyhow!("panik i hanterare")).into_response()
        }))
        .layer(TraceLayer::new_for_http())
}
