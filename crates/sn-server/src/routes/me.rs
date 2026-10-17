use std::collections::BTreeMap;

use axum::{Json, Router, routing::get};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState,
    auth::CurrentUser,
    authz::{Permission, role_label},
};

#[derive(Debug, Serialize, ToSchema)]
pub struct MeResponse {
    pub account_id: Uuid,
    pub person_id: Uuid,
    pub member_no: i32,
    pub name: String,
    pub auth_level: String,
    pub roles: Vec<RoleGrantDto>,
    /// Enheter som användaren kan arbeta i (kontextväljaren), med behörigheter.
    pub contexts: Vec<ContextDto>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RoleGrantDto {
    pub body_id: Uuid,
    pub body_name: String,
    pub body_kind: String,
    pub body_number: i32,
    pub role_key: String,
    pub role_label: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ContextDto {
    pub body_id: Uuid,
    pub body_name: String,
    pub body_kind: String,
    pub body_number: i32,
    pub roles: Vec<String>,
    pub permissions: Vec<Permission>,
}

/// Inloggad användare, roller och arbetskontexter.
#[utoipa::path(get, path = "/v1/me", tag = "me", responses((status = 200, body = MeResponse), (status = 401, body = crate::error::Problem)))]
pub async fn me(CurrentUser(p): CurrentUser) -> Json<MeResponse> {
    let roles = p
        .grants
        .iter()
        .map(|g| RoleGrantDto {
            body_id: g.body_id,
            body_name: g.body_name.clone(),
            body_kind: g.body_kind.clone(),
            body_number: g.body_number,
            role_key: g.role_key.clone(),
            role_label: role_label(&g.role_key).to_owned(),
        })
        .collect();

    let mut by_body: BTreeMap<(usize, String, Uuid), ContextDto> = BTreeMap::new();
    for g in &p.grants {
        let depth = g.body_path.matches('.').count();
        let entry = by_body.entry((depth, g.body_name.clone(), g.body_id)).or_insert_with(|| ContextDto {
            body_id: g.body_id,
            body_name: g.body_name.clone(),
            body_kind: g.body_kind.clone(),
            body_number: g.body_number,
            roles: Vec::new(),
            permissions: p.permissions_on(&g.body_path).into_iter().collect(),
        });
        entry.roles.push(role_label(&g.role_key).to_owned());
    }

    Json(MeResponse {
        account_id: p.account_id,
        person_id: p.person_id,
        member_no: p.member_no,
        name: p.display_name.clone(),
        auth_level: p.auth_level.clone(),
        roles,
        contexts: by_body.into_values().collect(),
    })
}

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/me", get(me))
}
