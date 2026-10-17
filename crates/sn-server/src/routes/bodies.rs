use std::time::Instant;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use sn_domain::{BodyKey, BodyKind, TroopType};
use sqlx::{Postgres, QueryBuilder};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::members::{csv_list, validate_in};
use crate::{
    AppState,
    auth::CurrentUser,
    authz::Permission,
    error::{ApiError, ApiResult},
};

/// Ordning mellan syskon i trädet: typ, därefter åldersgren och namn.
const KIND_ORDER: &str =
    "ARRAY['organisation','region','network','corps','district','corps_district','group','section','troop','patrol']";
const TROOP_ORDER: &str = "ARRAY['familj','baver','sparare','upptackare','aventyrare','utmanare','rover','annan']";
const MAX_NODES: i64 = 5000;

#[derive(Debug, Serialize, ToSchema)]
pub struct BodyDto {
    pub id: Uuid,
    pub kind: String,
    pub kind_label: String,
    pub name: String,
    /// Unikt per typ. Tillsammans med `kind` bildar det nyckeln `kind-number`, t.ex. `group-1001`.
    pub number: i32,
    pub troop_type: Option<String>,
    pub troop_type_label: Option<String>,
    pub min_age: Option<i16>,
    pub max_age: Option<i16>,
    pub municipality: Option<String>,
    pub meeting: Option<MeetingInfo>,
    /// Från roten ned till föräldern (brödsmulor).
    pub ancestors: Vec<CrumbDto>,
    pub counts: BodyCounts,
    /// Vad den inloggade får göra här.
    pub permissions: Vec<Permission>,
}

/// En överliggande enhet i brödsmulorna.
#[derive(Debug, Serialize, ToSchema)]
pub struct CrumbDto {
    pub id: Uuid,
    pub kind: String,
    pub kind_label: String,
    pub number: i32,
    pub name: String,
    /// Om den inloggade får öppna enheten. Annars visas bara namnet.
    pub accessible: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MeetingInfo {
    /// 1 = måndag … 7 = söndag.
    pub weekday: i16,
    pub start: String,
    pub duration_min: i16,
    pub location: Option<String>,
}

#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct BodyCounts {
    pub active: i64,
    pub leaders: i64,
    pub waiting: i64,
    pub pending: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BodyChildDto {
    pub id: Uuid,
    pub kind: String,
    pub kind_label: String,
    pub number: i32,
    pub name: String,
    pub troop_type: Option<String>,
    pub troop_type_label: Option<String>,
    pub active: i64,
    pub leaders: i64,
}

/// Filter och sortering för enheter under en enhet (organisationsöversikten).
#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DescendantsQuery {
    /// Antal nivåer under enheten (1 = närmaste barn). Utelämnas för alla nivåer.
    pub depth: Option<i32>,
    /// Fritext: namn, eller nummer om bara siffror.
    pub q: Option<String>,
    /// Kommaseparerade enhetstyper, t.ex. `group,troop`.
    pub kind: Option<String>,
    /// Kommaseparerade avdelningstyper, t.ex. `sparare,upptackare`.
    pub troop_type: Option<String>,
    /// `tree` (standard: nivå, typ, gren, namn) eller `name`, `number`, `scouts`, `leaders`, `waiting`, `pending`.
    /// Kommaseparerat, `-` för fallande.
    pub sort: Option<String>,
    /// Högst 5000, standard 500.
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// En enhet i organisationsöversikten, med antal medlemmar i hela delträdet.
#[derive(Debug, Serialize, ToSchema)]
pub struct OrgNodeDto {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub kind: String,
    pub kind_label: String,
    pub number: i32,
    pub name: String,
    pub troop_type: Option<String>,
    pub troop_type_label: Option<String>,
    pub municipality: Option<String>,
    /// Nivå under den efterfrågade enheten (1 = barn).
    pub depth: i32,
    /// Namnen på enheterna mellan den efterfrågade enheten och den här, uppifrån.
    pub path: Vec<String>,
    pub has_children: bool,
    pub counts: NodeCounts,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NodeCounts {
    /// Aktiva medlemmar som inte är ledare.
    pub scouts: i64,
    pub leaders: i64,
    pub waiting: i64,
    pub pending: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OrgNodePage {
    pub items: Vec<OrgNodeDto>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
    pub took_ms: f64,
}

#[derive(sqlx::FromRow)]
struct BodyRow {
    id: Uuid,
    kind: String,
    name: String,
    number: i32,
    troop_type: Option<String>,
    min_age: Option<i16>,
    max_age: Option<i16>,
    municipality: Option<String>,
    meeting_weekday: Option<i16>,
    meeting_start: Option<chrono::NaiveTime>,
    meeting_duration_min: Option<i16>,
    meeting_location: Option<String>,
    path: String,
}

#[derive(sqlx::FromRow)]
struct AncestorRow {
    id: Uuid,
    kind: String,
    number: i32,
    name: String,
    path: String,
}

#[derive(sqlx::FromRow)]
struct ChildRow {
    id: Uuid,
    kind: String,
    number: i32,
    name: String,
    troop_type: Option<String>,
    active: i64,
    leaders: i64,
}

#[derive(sqlx::FromRow)]
struct NodeRow {
    id: Uuid,
    parent_id: Option<Uuid>,
    kind: String,
    number: i32,
    name: String,
    troop_type: Option<String>,
    municipality: Option<String>,
    depth: i32,
    scouts: i64,
    leaders: i64,
    waiting: i64,
    pending: i64,
    total: i64,
    has_children: bool,
    path_names: Vec<String>,
}

fn kind_label(kind: &str) -> String {
    kind.parse::<BodyKind>().map_or_else(|_| kind.to_owned(), |k| k.label_sv().to_owned())
}

fn troop_label(t: Option<&str>) -> Option<String> {
    t.and_then(|t| t.parse::<TroopType>().ok()).map(|t| t.label_sv().to_owned())
}

/// Hämtar en enhet och kontrollerar behörigheten. `key` är enhetens UUID eller nyckeln `kind-number`
/// (t.ex. `group-1001`). Ger 404 om enheten saknas, nyckeln är ogiltig eller enheten inte får ses.
pub async fn load_visible_body(
    state: &AppState,
    user: &CurrentUser,
    key: &str,
    perm: Permission,
) -> ApiResult<(Uuid, String, String)> {
    let query = if let Ok(id) = key.parse::<Uuid>() {
        sqlx::query_as("SELECT id, kind, path::text FROM body WHERE id = $1").bind(id)
    } else {
        let BodyKey { kind, number } = key.parse().map_err(|_| ApiError::NotFound)?;
        sqlx::query_as("SELECT id, kind, path::text FROM body WHERE kind = $1 AND number = $2")
            .bind(kind.as_str())
            .bind(number)
    };
    let row: Option<(Uuid, String, String)> = query.fetch_optional(&state.db).await?;
    match row {
        Some((id, kind, path)) if user.0.can(perm, &path) => Ok((id, kind, path)),
        _ => Err(ApiError::NotFound),
    }
}

/// En enhet i organisationsträdet med nyckeltal och den inloggades behörigheter.
#[utoipa::path(get, path = "/v1/bodies/{id}", tag = "organisation",
    params(("id" = String, Path, description = "Enhetens id (UUID) eller nyckel `typ-nummer`, t.ex. `group-1001`")),
    responses((status = 200, body = BodyDto), (status = 404, body = crate::error::Problem)))]
pub async fn get_body(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(key): Path<String>,
) -> ApiResult<Json<BodyDto>> {
    let (id, ..) = load_visible_body(&state, &user, &key, Permission::BodyRead).await?;
    let b = sqlx::query_as::<_, BodyRow>(
        "SELECT id, kind, name, number, troop_type, min_age, max_age, municipality, meeting_weekday, meeting_start,
                meeting_duration_min, meeting_location, path::text AS path
         FROM body WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    let ancestors = sqlx::query_as::<_, AncestorRow>(
        "SELECT a.id, a.kind, a.number, a.name, a.path::text AS path
         FROM body a WHERE a.path @> $1::ltree AND a.path <> $1::ltree ORDER BY nlevel(a.path)",
    )
    .bind(&b.path)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|a| CrumbDto {
        accessible: user.0.can(Permission::BodyRead, &a.path),
        kind_label: kind_label(&a.kind),
        id: a.id,
        kind: a.kind,
        number: a.number,
        name: a.name,
    })
    .collect();

    let counts = sqlx::query_as::<_, BodyCounts>(
        "SELECT count(*) FILTER (WHERE membership_status = 'active') AS active,
                count(*) FILTER (WHERE membership_status = 'active' AND is_leader) AS leaders,
                count(*) FILTER (WHERE membership_status = 'waiting') AS waiting,
                count(*) FILTER (WHERE membership_status = 'pending') AS pending
         FROM member_current WHERE effective_path <@ $1::ltree",
    )
    .bind(&b.path)
    .fetch_one(&state.db)
    .await?;

    let meeting = match (b.meeting_weekday, b.meeting_start, b.meeting_duration_min) {
        (Some(weekday), Some(start), Some(duration_min)) => Some(MeetingInfo {
            weekday,
            start: start.format("%H:%M").to_string(),
            duration_min,
            location: b.meeting_location.clone(),
        }),
        _ => None,
    };

    Ok(Json(BodyDto {
        id: b.id,
        kind_label: kind_label(&b.kind),
        kind: b.kind,
        name: b.name,
        number: b.number,
        troop_type_label: troop_label(b.troop_type.as_deref()),
        troop_type: b.troop_type,
        min_age: b.min_age,
        max_age: b.max_age,
        municipality: b.municipality,
        meeting,
        ancestors,
        counts,
        permissions: user.0.permissions_on(&b.path).into_iter().collect(),
    }))
}

/// Underliggande enheter med antal medlemmar och ledare.
#[utoipa::path(get, path = "/v1/bodies/{id}/children", tag = "organisation",
    params(("id" = String, Path, description = "Enhetens id (UUID) eller nyckel `typ-nummer`, t.ex. `group-1001`")),
    responses((status = 200, body = [BodyChildDto]), (status = 404, body = crate::error::Problem)))]
pub async fn get_children(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(key): Path<String>,
) -> ApiResult<Json<Vec<BodyChildDto>>> {
    let (id, ..) = load_visible_body(&state, &user, &key, Permission::BodyRead).await?;
    let rows = sqlx::query_as::<_, ChildRow>(
        "SELECT c.id, c.kind, c.number, c.name, c.troop_type,
                (SELECT count(*) FROM member_current mc WHERE mc.effective_path <@ c.path AND mc.membership_status = 'active') AS active,
                (SELECT count(*) FROM member_current mc WHERE mc.effective_path <@ c.path AND mc.membership_status = 'active' AND mc.is_leader) AS leaders
         FROM body c WHERE c.parent_id = $1 AND c.status = 'active'
         ORDER BY c.kind, array_position(ARRAY['familj','baver','sparare','upptackare','aventyrare','utmanare','rover','annan'], c.troop_type), c.name",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|c| BodyChildDto {
                id: c.id,
                kind_label: kind_label(&c.kind),
                kind: c.kind,
                number: c.number,
                name: c.name,
                troop_type_label: troop_label(c.troop_type.as_deref()),
                troop_type: c.troop_type,
                active: c.active,
                leaders: c.leaders,
            })
            .collect(),
    ))
}

/// Översätter sorteringsnycklar för enheter till SQL. Endast vitlistade uttryck används.
fn node_order(sort: Option<&str>) -> ApiResult<String> {
    let tree =
        format!("depth, array_position({KIND_ORDER}, kind), array_position({TROOP_ORDER}, troop_type), lower(name)");
    let mut parts = Vec::new();
    for key in csv_list(sort) {
        let (desc, key) = key.strip_prefix('-').map_or((false, key.as_str()), |k| (true, k));
        let dir = if desc { "DESC" } else { "ASC" };
        parts.push(match key {
            "tree" => tree.clone(),
            "name" => format!("lower(name) {dir}"),
            "number" => format!("number {dir}"),
            k @ ("scouts" | "leaders" | "waiting" | "pending") => format!("{k} {dir}"),
            other => return Err(ApiError::BadRequest(format!("okänd sorteringsnyckel: {other}"))),
        });
    }
    if parts.is_empty() {
        parts.push(tree);
    }
    parts.push("lower(name), id".into());
    Ok(parts.join(", "))
}

/// Enheter under en enhet, för organisationsöversikten: som träd (med `depth`) eller som filtrerad,
/// sorterad lista. Antalen gäller hela delträdet under varje enhet.
#[utoipa::path(get, path = "/v1/bodies/{id}/descendants", tag = "organisation",
    params(("id" = String, Path, description = "Enhetens id (UUID) eller nyckel `typ-nummer`, t.ex. `district-1`"), DescendantsQuery),
    responses((status = 200, body = OrgNodePage), (status = 400, body = crate::error::Problem), (status = 404, body = crate::error::Problem)))]
pub async fn get_descendants(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(key): Path<String>,
    Query(q): Query<DescendantsQuery>,
) -> ApiResult<Json<OrgNodePage>> {
    let started = Instant::now();
    let (root_id, ..) = load_visible_body(&state, &user, &key, Permission::BodyRead).await?;

    let kinds = csv_list(q.kind.as_deref());
    validate_in(&kinds, &BodyKind::ALL.map(BodyKind::as_str), "kind")?;
    let troop_types = csv_list(q.troop_type.as_deref());
    validate_in(&troop_types, &TroopType::ALL.map(TroopType::as_str), "troop_type")?;
    let order = node_order(q.sort.as_deref())?;
    let limit = q.limit.unwrap_or(500).clamp(1, MAX_NODES);
    let offset = q.offset.unwrap_or(0).max(0);

    // Antal per enhet räknas en gång per medlemmens placering och summeras sedan uppåt längs
    // sökvägen (subpath). Det ger en likhetsjoin i stället för en ltree-jämförelse per enhet och medlem.
    let mut qb = QueryBuilder::<Postgres>::new("WITH root AS (SELECT path FROM body WHERE id = ");
    qb.push_bind(root_id);
    qb.push(
        "), agg AS (
             SELECT mc.effective_path AS p,
                    count(*) FILTER (WHERE mc.membership_status = 'active' AND NOT mc.is_leader) AS scouts,
                    count(*) FILTER (WHERE mc.membership_status = 'active' AND mc.is_leader) AS leaders,
                    count(*) FILTER (WHERE mc.membership_status = 'waiting') AS waiting,
                    count(*) FILTER (WHERE mc.membership_status = 'pending') AS pending
             FROM member_current mc, root r WHERE mc.effective_path <@ r.path GROUP BY 1
         ), rolled AS (
             SELECT subpath(a.p, 0, i) AS path, sum(a.scouts)::int8 AS scouts, sum(a.leaders)::int8 AS leaders,
                    sum(a.waiting)::int8 AS waiting, sum(a.pending)::int8 AS pending
             FROM agg a, root r, generate_series(nlevel(r.path) + 1, nlevel(a.p)) AS i GROUP BY 1
         ), nodes AS (
             SELECT b.id, b.parent_id, b.kind, b.number, b.name, b.troop_type, b.municipality, b.path,
                    nlevel(b.path) - nlevel(r.path) AS depth,
                    coalesce(c.scouts, 0) AS scouts, coalesce(c.leaders, 0) AS leaders,
                    coalesce(c.waiting, 0) AS waiting, coalesce(c.pending, 0) AS pending
             FROM body b CROSS JOIN root r LEFT JOIN rolled c ON c.path = b.path
             WHERE b.path <@ r.path AND b.path <> r.path AND b.status = 'active'",
    );
    if let Some(depth) = q.depth {
        qb.push(" AND nlevel(b.path) - nlevel(r.path) <= ").push_bind(depth.clamp(1, 20));
    }
    if let Some(text) = q.q.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        if text.chars().all(|c| c.is_ascii_digit()) {
            qb.push(" AND b.number::text LIKE ").push_bind(format!("{text}%"));
        } else {
            qb.push(" AND lower(b.name) LIKE ").push_bind(format!("%{}%", text.to_lowercase()));
        }
    }
    if !kinds.is_empty() {
        qb.push(" AND b.kind = ANY(").push_bind(&kinds).push(")");
    }
    if !troop_types.is_empty() {
        qb.push(" AND b.troop_type = ANY(").push_bind(&troop_types).push(")");
    }
    qb.push("), page AS (SELECT n.*, count(*) OVER () AS total FROM nodes n ORDER BY ").push(&order);
    qb.push(" LIMIT ").push_bind(limit).push(" OFFSET ").push_bind(offset);
    qb.push(
        ") SELECT p.id, p.parent_id, p.kind, p.number, p.name, p.troop_type, p.municipality, p.depth,
                  p.scouts, p.leaders, p.waiting, p.pending, p.total,
                  EXISTS (SELECT 1 FROM body ch WHERE ch.parent_id = p.id AND ch.status = 'active') AS has_children,
                  array(SELECT a.name FROM body a, root r
                        WHERE a.path @> p.path AND a.path <> p.path AND nlevel(a.path) > nlevel(r.path)
                        ORDER BY nlevel(a.path)) AS path_names
           FROM page p ORDER BY ",
    )
    .push(&order);

    let rows: Vec<NodeRow> = qb.build_query_as().fetch_all(&state.db).await?;
    let total = rows.first().map_or(0, |r| r.total);
    let items = rows
        .into_iter()
        .map(|r| OrgNodeDto {
            id: r.id,
            parent_id: r.parent_id,
            kind_label: kind_label(&r.kind),
            kind: r.kind,
            number: r.number,
            name: r.name,
            troop_type_label: troop_label(r.troop_type.as_deref()),
            troop_type: r.troop_type,
            municipality: r.municipality,
            depth: r.depth,
            path: r.path_names,
            has_children: r.has_children,
            counts: NodeCounts { scouts: r.scouts, leaders: r.leaders, waiting: r.waiting, pending: r.pending },
        })
        .collect();

    Ok(Json(OrgNodePage { items, total, limit, offset, took_ms: started.elapsed().as_secs_f64() * 1000.0 }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/bodies/{id}", get(get_body))
        .route("/v1/bodies/{id}/children", get(get_children))
        .route("/v1/bodies/{id}/descendants", get(get_descendants))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_order_whitelists_keys() {
        assert!(node_order(None).is_ok_and(|s| s.starts_with("depth, array_position(")));
        assert!(node_order(Some("-scouts,name")).is_ok_and(|s| s.starts_with("scouts DESC, lower(name) ASC")));
        assert!(node_order(Some("scouts; DROP TABLE body")).is_err());
    }
}
