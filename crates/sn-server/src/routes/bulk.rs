//! Massåtgärder i en kår: flytta medlemmar till avdelning/patrull och ta in från väntelistan.
//! Varje åtgärd kan köras som torrkörning (`dry_run`), som visar exakt vad som skulle hända per person.

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::post,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{audit, bodies::load_visible_body};
use crate::{
    AppState,
    auth::CurrentUser,
    authz::{Permission, role_label},
    error::{ApiError, ApiResult},
};

const MAX_ROWS: usize = 500;

#[derive(Debug, Deserialize, ToSchema)]
pub struct BulkRequest {
    /// `move` (flytta aktiva medlemmar) eller `admit` (ta in från väntelista eller godkänn väntande).
    pub action: String,
    pub person_ids: Vec<Uuid>,
    /// Avdelningen i kåren som personerna ska placeras i.
    pub troop_id: Uuid,
    /// Patrull i avdelningen (valfritt).
    pub patrol_id: Option<Uuid>,
    /// Visa vad som skulle hända utan att ändra något.
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BulkRow {
    pub person_id: Uuid,
    pub name: String,
    /// Nuvarande placering eller status, t.ex. "Vargarna / Patrull Ugglan" eller "Väntelista".
    pub from: String,
    pub to: String,
    /// `ok`, `warning` (genomförs, men se meddelandet) eller `skip` (genomförs inte).
    pub outcome: String,
    pub message: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BulkResult {
    pub action: String,
    pub dry_run: bool,
    /// Antal som ändras (eller skulle ändras vid torrkörning).
    pub changed: usize,
    pub skipped: usize,
    pub rows: Vec<BulkRow>,
}

#[derive(sqlx::FromRow)]
struct Candidate {
    id: Uuid,
    first_name: String,
    last_name: String,
    membership_id: Option<Uuid>,
    membership_status: Option<String>,
    troop_id: Option<Uuid>,
    troop_name: Option<String>,
    patrol_id: Option<Uuid>,
    patrol_name: Option<String>,
    is_leader: Option<bool>,
    roles: Vec<String>,
}

/// Utför en massåtgärd i kåren. Kräver `members_manage` i kåren.
#[utoipa::path(post, path = "/v1/bodies/{id}/bulk", tag = "members",
    params(("id" = String, Path, description = "Kårens id eller nyckel, t.ex. `group-1001`")),
    request_body = BulkRequest,
    responses((status = 200, body = BulkResult), (status = 400, body = crate::error::Problem), (status = 403, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
#[expect(
    clippy::too_many_lines,
    reason = "kontroll per rad och genomförande hålls ihop för att vara lätta att granska"
)]
pub async fn bulk(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(key): Path<String>,
    Json(req): Json<BulkRequest>,
) -> ApiResult<Json<BulkResult>> {
    let (group_id, kind, path) = load_visible_body(&state, &user, &key, Permission::MembersRead).await?;
    if kind != "group" {
        return Err(ApiError::BadRequest("massåtgärder görs i en kår".into()));
    }
    if !user.0.can(Permission::MembersManage, &path) {
        return Err(ApiError::Forbidden);
    }
    if !matches!(req.action.as_str(), "move" | "admit") {
        return Err(ApiError::BadRequest(format!("okänd åtgärd: {}", req.action)));
    }
    if req.person_ids.is_empty() || req.person_ids.len() > MAX_ROWS {
        return Err(ApiError::BadRequest(format!("välj 1–{MAX_ROWS} personer")));
    }

    // Målet måste vara en avdelning i kåren, och patrullen en patrull i den avdelningen.
    let troop: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM body WHERE id = $1 AND kind = 'troop' AND parent_id = $2 AND status = 'active'",
    )
    .bind(req.troop_id)
    .bind(group_id)
    .fetch_optional(&state.db)
    .await?;
    let Some((troop_name,)) = troop else {
        return Err(ApiError::BadRequest("avdelningen finns inte i kåren".into()));
    };
    let patrol_name = match req.patrol_id {
        Some(pid) => {
            let p: Option<(String,)> = sqlx::query_as(
                "SELECT name FROM body WHERE id = $1 AND kind = 'patrol' AND parent_id = $2 AND status = 'active'",
            )
            .bind(pid)
            .bind(req.troop_id)
            .fetch_optional(&state.db)
            .await?;
            Some(p.ok_or_else(|| ApiError::BadRequest("patrullen finns inte i avdelningen".into()))?.0)
        }
        None => None,
    };
    let to = patrol_name.as_ref().map_or_else(|| troop_name.clone(), |p| format!("{troop_name} / {p}"));

    let candidates = sqlx::query_as::<_, Candidate>(
        "SELECT p.id, p.first_name, p.last_name, m.id AS membership_id, m.status AS membership_status,
                t.id AS troop_id, t.name AS troop_name, pa.id AS patrol_id, pa.name AS patrol_name, t.is_leader,
                array(SELECT ra.role_key FROM role_assignment ra JOIN body rb ON rb.id = ra.body_id
                      WHERE ra.person_id = p.id AND ra.valid @> current_date AND rb.kind IN ('troop', 'patrol')) AS roles
         FROM unnest($1::uuid[]) AS ids(id) JOIN person p ON p.id = ids.id
         LEFT JOIN membership m ON m.person_id = p.id AND m.group_id = $2 AND upper_inf(m.valid)
         LEFT JOIN LATERAL (
             SELECT b.id, b.name, pl.is_leader FROM placement pl JOIN body b ON b.id = pl.body_id
             WHERE pl.person_id = p.id AND b.kind = 'troop' AND b.parent_id = $2 AND upper_inf(pl.valid)
             ORDER BY lower(pl.valid) DESC LIMIT 1) t ON true
         LEFT JOIN LATERAL (
             SELECT b.id, b.name FROM placement pl JOIN body b ON b.id = pl.body_id
             WHERE pl.person_id = p.id AND b.kind = 'patrol' AND b.parent_id = t.id AND upper_inf(pl.valid)
             ORDER BY lower(pl.valid) DESC LIMIT 1) pa ON true
         ORDER BY p.last_name, p.first_name",
    )
    .bind(&req.person_ids)
    .bind(group_id)
    .fetch_all(&state.db)
    .await?;

    let mut rows = Vec::with_capacity(candidates.len());
    let mut todo: Vec<(Uuid, Option<Uuid>, bool)> = Vec::new();
    for c in &candidates {
        let name = format!("{} {}", c.first_name, c.last_name);
        let from = match (c.membership_status.as_deref(), &c.troop_name, &c.patrol_name) {
            (Some("waiting"), ..) => "Väntelista".to_owned(),
            (Some("pending"), ..) => "Väntar på godkännande".to_owned(),
            (_, Some(t), Some(p)) => format!("{t} / {p}"),
            (_, Some(t), None) => t.clone(),
            (Some(_), None, _) => "Ingen avdelning".to_owned(),
            (None, ..) => "Inte medlem i kåren".to_owned(),
        };
        let mut row = BulkRow { person_id: c.id, name, from, to: to.clone(), outcome: "ok".into(), message: None };
        match (req.action.as_str(), c.membership_status.as_deref()) {
            (_, None) => skip(&mut row, "Inte medlem i kåren"),
            ("move", Some("active")) => {
                if c.troop_id == Some(req.troop_id) && c.patrol_id == req.patrol_id {
                    skip(&mut row, "Är redan placerad där");
                } else {
                    if !c.roles.is_empty() {
                        row.outcome = "warning".into();
                        let roles: Vec<&str> = c.roles.iter().map(|r| role_label(r)).collect();
                        row.message = Some(format!(
                            "Har rollen {} i nuvarande avdelning. Rollen flyttas inte.",
                            roles.join(", ")
                        ));
                    }
                    todo.push((c.id, c.membership_id, c.is_leader.unwrap_or(false)));
                }
            }
            ("move", Some(_)) => skip(&mut row, "Är inte aktiv medlem. Använd \"Ta in\"."),
            ("admit", Some("waiting" | "pending")) => todo.push((c.id, c.membership_id, false)),
            ("admit", Some(_)) => skip(&mut row, "Är redan aktiv medlem. Använd \"Flytta\"."),
            _ => skip(&mut row, "Kan inte hanteras"),
        }
        rows.push(row);
    }
    // Id:n som inte hittades alls.
    for id in &req.person_ids {
        if !candidates.iter().any(|c| c.id == *id) {
            rows.push(BulkRow {
                person_id: *id,
                name: "Okänd person".into(),
                from: String::new(),
                to: to.clone(),
                outcome: "skip".into(),
                message: Some("Personen finns inte".into()),
            });
        }
    }

    if !req.dry_run && !todo.is_empty() {
        let persons: Vec<Uuid> = todo.iter().map(|t| t.0).collect();
        let mut tx = state.db.begin().await?;
        if req.action == "admit" {
            sqlx::query(
                "UPDATE membership SET status = 'active', valid = daterange(current_date, NULL), confirmed_at = current_date
                 WHERE person_id = ANY($1) AND group_id = $2 AND upper_inf(valid) AND status IN ('waiting', 'pending')",
            )
            .bind(&persons)
            .bind(group_id)
            .execute(&mut *tx)
            .await?;
            sqlx::query("UPDATE person SET person_status = 'active' WHERE id = ANY($1) AND person_status IN ('new', 'unregistered')")
                .bind(&persons)
                .execute(&mut *tx)
                .await?;
        }
        // Avsluta nuvarande placeringar i kåren. Placeringar som började i dag tas bort i stället.
        sqlx::query(
            "DELETE FROM placement pl USING body b
             WHERE b.id = pl.body_id AND pl.person_id = ANY($1) AND b.path <@ (SELECT path FROM body WHERE id = $2)
               AND b.kind IN ('troop', 'patrol') AND upper_inf(pl.valid) AND lower(pl.valid) >= current_date",
        )
        .bind(&persons)
        .bind(group_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE placement pl SET valid = daterange(lower(pl.valid), current_date) FROM body b
             WHERE b.id = pl.body_id AND pl.person_id = ANY($1) AND b.path <@ (SELECT path FROM body WHERE id = $2)
               AND b.kind IN ('troop', 'patrol') AND upper_inf(pl.valid)",
        )
        .bind(&persons)
        .bind(group_id)
        .execute(&mut *tx)
        .await?;
        let leaders: Vec<bool> = todo.iter().map(|t| t.2).collect();
        sqlx::query(
            "INSERT INTO placement (person_id, body_id, is_leader, valid)
             SELECT p, $2, l, daterange(current_date, NULL) FROM unnest($1::uuid[], $3::bool[]) AS x(p, l)",
        )
        .bind(&persons)
        .bind(req.troop_id)
        .bind(&leaders)
        .execute(&mut *tx)
        .await?;
        if let Some(pid) = req.patrol_id {
            sqlx::query(
                "INSERT INTO placement (person_id, body_id, is_leader, valid)
                 SELECT p, $2, false, daterange(current_date, NULL) FROM unnest($1::uuid[]) AS p",
            )
            .bind(&persons)
            .bind(pid)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        audit(
            &state.db,
            user.0.account_id,
            &format!("bulk.{}", req.action),
            "body",
            group_id,
            serde_json::json!({ "persons": persons, "troop": req.troop_id, "patrol": req.patrol_id, "to": to }),
        )
        .await;
    }

    let skipped = rows.iter().filter(|r| r.outcome == "skip").count();
    Ok(Json(BulkResult { action: req.action, dry_run: req.dry_run, changed: todo.len(), skipped, rows }))
}

fn skip(row: &mut BulkRow, msg: &str) {
    row.outcome = "skip".into();
    row.message = Some(msg.into());
}

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/bodies/{id}/bulk", post(bulk))
}
