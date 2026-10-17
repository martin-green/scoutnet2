//! Sparade vyer: namngivna listfrågor per enhet, egna eller delade med alla som ser listan.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::bodies::load_visible_body;
use crate::{
    AppState,
    auth::CurrentUser,
    authz::Permission,
    error::{ApiError, ApiResult},
};

/// Parametrar som inte hör hemma i en sparad vy (paginering).
const DROPPED_PARAMS: [&str; 1] = ["offset"];

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct SavedView {
    pub id: Uuid,
    /// `members`, `waiting` eller `organisation`.
    pub list: String,
    pub name: String,
    /// Frågan som i adressfältet, t.ex. `troop_type=sparare&sort=-birth_date`.
    pub query: String,
    pub shared: bool,
    pub owner_name: String,
    /// Om den inloggade äger vyn (och därmed kan ta bort den).
    pub mine: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ViewQuery {
    /// `members`, `waiting` eller `organisation`.
    pub list: String,
}

fn list_permission(list: &str) -> ApiResult<Permission> {
    match list {
        "members" | "waiting" => Ok(Permission::MembersRead),
        "organisation" => Ok(Permission::BodyRead),
        other => Err(ApiError::BadRequest(format!("okänd lista: {other}"))),
    }
}

/// Normaliserar frågan: tar bort paginering och tomma värden och sorterar parametrarna.
fn normalize_query(query: &str) -> ApiResult<String> {
    let query = query.trim().trim_start_matches('?');
    if query.len() > 2000 {
        return Err(ApiError::BadRequest("frågan är för lång".into()));
    }
    let mut pairs: Vec<(String, String)> = query
        .split('&')
        .filter(|p| !p.is_empty())
        .filter_map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (!v.is_empty() && !DROPPED_PARAMS.contains(&k)).then(|| (k.to_owned(), v.to_owned()))
        })
        .collect();
    if pairs.iter().any(|(k, _)| !k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')) {
        return Err(ApiError::BadRequest("ogiltig parameter i frågan".into()));
    }
    pairs.sort();
    Ok(pairs.into_iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("&"))
}

/// Egna och delade vyer för en lista i enheten.
#[utoipa::path(get, path = "/v1/bodies/{id}/views", tag = "views",
    params(("id" = String, Path, description = "Enhetens id eller nyckel, t.ex. `group-1001`"), ViewQuery),
    responses((status = 200, body = [SavedView]), (status = 404, body = crate::error::Problem)))]
pub async fn list_views(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(key): Path<String>,
    Query(q): Query<ViewQuery>,
) -> ApiResult<Json<Vec<SavedView>>> {
    let (body_id, ..) = load_visible_body(&state, &user, &key, list_permission(&q.list)?).await?;
    let views = sqlx::query_as::<_, SavedView>(
        "SELECT v.id, v.list, v.name, v.query, v.shared, p.first_name || ' ' || p.last_name AS owner_name,
                v.owner_account_id = $3 AS mine, v.created_at
         FROM saved_view v JOIN account a ON a.id = v.owner_account_id JOIN person p ON p.id = a.person_id
         WHERE v.body_id = $1 AND v.list = $2 AND (v.shared OR v.owner_account_id = $3)
         ORDER BY v.shared, lower(v.name)",
    )
    .bind(body_id)
    .bind(&q.list)
    .bind(user.0.account_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(views))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateView {
    pub list: String,
    pub name: String,
    pub query: String,
    /// Dela med alla som ser listan i enheten.
    #[serde(default)]
    pub shared: bool,
}

/// Sparar en vy. Finns redan en egen vy med samma namn ersätts den.
#[utoipa::path(post, path = "/v1/bodies/{id}/views", tag = "views",
    params(("id" = String, Path, description = "Enhetens id eller nyckel")),
    request_body = CreateView,
    responses((status = 201, body = SavedView), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn create_view(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(key): Path<String>,
    Json(req): Json<CreateView>,
) -> ApiResult<(StatusCode, Json<SavedView>)> {
    let (body_id, ..) = load_visible_body(&state, &user, &key, list_permission(&req.list)?).await?;
    let name = req.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(ApiError::BadRequest("namnet ska vara 1–80 tecken".into()));
    }
    let query = normalize_query(&req.query)?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO saved_view (owner_account_id, body_id, list, name, query, shared) VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (owner_account_id, body_id, list, lower(name))
           DO UPDATE SET name = EXCLUDED.name, query = EXCLUDED.query, shared = EXCLUDED.shared, created_at = now()
         RETURNING id",
    )
    .bind(user.0.account_id)
    .bind(body_id)
    .bind(&req.list)
    .bind(name)
    .bind(&query)
    .bind(req.shared)
    .fetch_one(&state.db)
    .await?;
    let view = sqlx::query_as::<_, SavedView>(
        "SELECT v.id, v.list, v.name, v.query, v.shared, p.first_name || ' ' || p.last_name AS owner_name, true AS mine, v.created_at
         FROM saved_view v JOIN account a ON a.id = v.owner_account_id JOIN person p ON p.id = a.person_id WHERE v.id = $1",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;
    Ok((StatusCode::CREATED, Json(view)))
}

/// Tar bort en egen vy. Delade vyer kan även tas bort av den som får hantera medlemmar i enheten.
#[utoipa::path(delete, path = "/v1/views/{id}", tag = "views",
    params(("id" = Uuid, Path, description = "Vyns id")),
    responses((status = 204), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn delete_view(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let row: Option<(Uuid, bool, String, String)> = sqlx::query_as(
        "SELECT v.owner_account_id, v.shared, v.list, b.path::text FROM saved_view v JOIN body b ON b.id = v.body_id WHERE v.id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let Some((owner, shared, list, path)) = row else { return Err(ApiError::NotFound) };
    let mine = owner == user.0.account_id;
    let visible = mine || (shared && user.0.can(list_permission(&list)?, &path));
    if !visible {
        return Err(ApiError::NotFound);
    }
    if !mine && !user.0.can(Permission::MembersManage, &path) {
        return Err(ApiError::Forbidden);
    }
    sqlx::query("DELETE FROM saved_view WHERE id = $1").bind(id).execute(&state.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/bodies/{id}/views", get(list_views).post(create_view))
        .route("/v1/views/{id}", delete(delete_view))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_are_normalized() {
        assert_eq!(
            normalize_query("?sort=name&offset=100&troop_type=sparare&q=").expect("ok"),
            "sort=name&troop_type=sparare"
        );
        assert!(normalize_query("a b=1").is_err());
        assert!(normalize_query(&"x=1&".repeat(600)).is_err());
    }
}
