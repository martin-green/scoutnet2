//! Moduler: avgränsade funktionsområden ovanpå kärnan (se `docs/06-moduler.md`).
//!
//! Kärnan äger organisationen, personer, medlemskap, behörighet och revisionsloggen. En modul,
//! t.ex. Närvaro eller Bidrag, har ett manifest (namn, beroenden, menyval), egna API-anrop med
//! egen OpenAPI-del och slås på eller av per förbund, distrikt eller kår. Ett val gäller enheten
//! och allt under den. Ett förbund eller distrikt kan låsa sitt val.
//!
//! En ny modul: skriv en typ som implementerar [`Module`], lägg den i [`MODULES`] och lägg
//! modulens sidor i webbappen. Menyval, spärr, aktivering och API-dokumentation följer med.

use std::collections::HashMap;

use axum::{
    Extension, Json, Router,
    extract::{FromRequestParts, MatchedPath, Path, Request, State},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, put},
};
use serde::{Deserialize, Serialize};
use sn_domain::BodyKey;
use utoipa::{OpenApi, ToSchema};
use uuid::Uuid;

use crate::{
    AppState,
    auth::CurrentUser,
    authz::{Permission, Principal},
    error::{ApiError, ApiResult},
    routes::{audit, bodies::load_visible_body},
};

pub mod attendance;
pub mod grants;
pub mod integrations;
pub mod statistics;

// ---------------------------------------------------------------------------------------------
// Kontraktet

/// Om modulen går att använda eller bara är planerad (visas i katalogen men kan inte slås på).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    Available,
    Planned,
}

/// Det en modul berättar om sig själv. Allt är statiskt, så att katalogen kan visas och
/// kontrolleras utan att modulen körs.
#[derive(Debug)]
pub struct Manifest {
    /// Stabilt id, används i API och databas (`module_setting.module_id`).
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub icon: &'static str,
    pub availability: Availability,
    /// Läge när varken enheten eller någon överliggande enhet har gjort ett val.
    pub default_enabled: bool,
    /// Moduler som måste vara på. Modulen räknas som av annars.
    pub depends_on: &'static [&'static str],
    pub nav: &'static [NavEntry],
}

/// Ett menyval som modulen lägger till för en enhet.
#[derive(Debug)]
pub struct NavEntry {
    /// Sökvägen under enheten i webbappen, t.ex. `narvaro` i `/kar/1001/narvaro`.
    pub segment: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
    /// Enhetstyper där menyvalet visas.
    pub kinds: &'static [&'static str],
    /// Behörighet som krävs på enheten för att menyvalet ska visas.
    pub permission: Permission,
}

/// En inbyggd modul.
pub trait Module: Sync {
    fn manifest(&self) -> &'static Manifest;
    /// Modulens API-anrop. Anrop under `/v1/bodies/{id}/…` spärras automatiskt när modulen är av
    /// för enheten. Andra anrop (t.ex. `/v1/meetings/{id}`) kontrollerar själva med [`require`].
    fn router(&self) -> Router<AppState>;
    /// Modulens del av OpenAPI-specifikationen.
    fn openapi(&self) -> utoipa::openapi::OpenApi;
}

/// Inbyggda moduler i menyordning. En modul ska stå efter de moduler den beror på.
pub static MODULES: &[&dyn Module] =
    &[&attendance::Attendance, &grants::Grants, &statistics::Statistics, &integrations::Integrations];

/// Moduler som planeras men inte finns ännu. De visas i katalogen så att kårer ser vad som kommer.
static PLANNED: &[Manifest] = &[
    Manifest {
        id: "billing",
        name: "Fakturering",
        description: "Terminsavgifter och fakturor till medlemmar och vårdnadshavare, med påminnelser, \
                      avstämning mot Bankgiro och Swish och export till bokföringen. Tar över avgiftsfälten från kärnan.",
        icon: "💳",
        availability: Availability::Planned,
        default_enabled: false,
        depends_on: &[],
        nav: &[],
    },
    Manifest {
        id: "mailing_lists",
        name: "E-postlistor",
        description: "Listor som uppdateras av sig själva utifrån avdelning, roll och ålder, t.ex. \"alla \
                      vårdnadshavare i Spårhundarna\". Synkas till Google Workspace eller en e-posttjänst.",
        icon: "✉️",
        availability: Availability::Planned,
        default_enabled: false,
        depends_on: &[],
        nav: &[],
    },
    Manifest {
        id: "activities",
        name: "Aktiviteter",
        description: "Arrangemang och läger med anmälan, deltagarlistor, specialkost och betalning via \
                      Fakturering. Öppna för andra kårer och distrikt.",
        icon: "🏕️",
        availability: Availability::Planned,
        default_enabled: false,
        depends_on: &[],
        nav: &[],
    },
];

/// Alla moduler, inbyggda först och sedan planerade.
pub fn catalog() -> impl Iterator<Item = &'static Manifest> {
    MODULES.iter().map(|m| m.manifest()).chain(PLANNED.iter())
}

fn manifest(id: &str) -> Option<&'static Manifest> {
    catalog().find(|m| m.id == id)
}

fn name_of(id: &str) -> &'static str {
    manifest(id).map_or("okänd modul", |m| m.name)
}

/// Enhetstyper där moduler kan väljas. Avdelningar och patruller följer sin kår.
fn is_settable(kind: &str) -> bool {
    !matches!(kind, "troop" | "patrol" | "section")
}

// ---------------------------------------------------------------------------------------------
// Aktivering: arv nedåt i trädet, låsning och beroenden

/// Ett val i `module_setting` för enheten eller en överliggande enhet.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Setting {
    pub module_id: String,
    pub enabled: bool,
    pub locked: bool,
    pub body_id: Uuid,
    pub body_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Default,
    Inherited(String),
    Own,
}

/// En moduls läge för en enhet.
#[derive(Debug, Clone)]
pub struct Resolved {
    /// På efter arv, låsning och beroenden.
    pub enabled: bool,
    /// Valet (eller standardläget) utan hänsyn till beroenden.
    pub chosen: bool,
    pub source: Source,
    /// Överliggande enhet som har låst valet.
    pub locked_by: Option<String>,
    /// Enheten har själv låst valet för underliggande enheter.
    pub locked_here: bool,
    /// Beroenden som inte är på.
    pub missing: Vec<&'static str>,
}

/// Räknar fram läget för alla moduler. `settings` ska vara ordnade från roten ned till enheten.
/// Det närmaste valet gäller, utom när en överliggande enhet har låst: då gäller det översta låsta valet.
pub fn resolve(settings: &[Setting], body_id: Uuid) -> HashMap<&'static str, Resolved> {
    let mut out: HashMap<&'static str, Resolved> = HashMap::new();
    for m in catalog() {
        let available = m.availability == Availability::Available;
        let mut r = Resolved {
            enabled: false,
            chosen: available && m.default_enabled,
            source: Source::Default,
            locked_by: None,
            locked_here: false,
            missing: Vec::new(),
        };
        if available {
            for s in settings.iter().filter(|s| s.module_id == m.id) {
                r.chosen = s.enabled;
                if s.body_id == body_id {
                    r.source = Source::Own;
                    r.locked_here = s.locked;
                } else {
                    r.source = Source::Inherited(s.body_name.clone());
                    if s.locked {
                        r.locked_by = Some(s.body_name.clone());
                        break;
                    }
                }
            }
        }
        out.insert(m.id, r);
    }
    // Katalogen är ordnad så att beroenden kommer först, så ett varv räcker.
    for m in catalog() {
        let missing: Vec<&'static str> =
            m.depends_on.iter().copied().filter(|d| !out.get(d).is_some_and(|r| r.enabled)).collect();
        if let Some(r) = out.get_mut(m.id) {
            r.enabled = r.chosen && missing.is_empty();
            r.missing = missing;
        }
    }
    out
}

/// Val för enheten och alla överliggande enheter, från roten och nedåt.
pub async fn load_settings(db: &sqlx::PgPool, path: &str) -> ApiResult<Vec<Setting>> {
    Ok(sqlx::query_as::<_, Setting>(
        "SELECT ms.module_id, ms.enabled, ms.locked, b.id AS body_id, b.name AS body_name
         FROM module_setting ms JOIN body b ON b.id = ms.body_id
         WHERE b.path @> $1::ltree ORDER BY nlevel(b.path)",
    )
    .bind(path)
    .fetch_all(db)
    .await?)
}

/// Om modulen är på för enheten med sökvägen `path`.
pub async fn is_active(state: &AppState, module: &str, path: &str) -> ApiResult<bool> {
    let resolved = resolve(&load_settings(&state.db, path).await?, Uuid::nil());
    Ok(resolved.get(module).is_some_and(|r| r.enabled))
}

/// Felet `module_disabled` om modulen är av för enheten.
pub async fn require(state: &AppState, module: &str, path: &str) -> ApiResult<()> {
    if is_active(state, module, path).await? { Ok(()) } else { Err(disabled(state, module, path).await) }
}

async fn disabled(state: &AppState, module: &str, path: &str) -> ApiError {
    let body: Option<String> = sqlx::query_scalar("SELECT name FROM body WHERE path = $1::ltree")
        .bind(path)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
    ApiError::ModuleDisabled(format!(
        "Modulen {} är inte aktiverad för {}.",
        name_of(module),
        body.as_deref().unwrap_or("enheten")
    ))
}

/// Ett menyval för den inloggade på en enhet.
#[derive(Debug, Serialize, ToSchema)]
pub struct NavItem {
    pub module: String,
    pub segment: String,
    pub label: String,
    pub icon: String,
}

/// Aktiva moduler och menyval för den inloggade på en enhet.
pub async fn for_body(
    state: &AppState,
    principal: &Principal,
    kind: &str,
    path: &str,
) -> ApiResult<(Vec<String>, Vec<NavItem>)> {
    let resolved = resolve(&load_settings(&state.db, path).await?, Uuid::nil());
    let active: Vec<&'static Manifest> = catalog().filter(|m| resolved.get(m.id).is_some_and(|r| r.enabled)).collect();
    let nav = active
        .iter()
        .flat_map(|m| m.nav.iter().map(move |n| (m.id, n)))
        .filter(|(_, n)| n.kinds.contains(&kind) && principal.can(n.permission, path))
        .map(|(id, n)| NavItem {
            module: id.to_owned(),
            segment: n.segment.to_owned(),
            label: n.label.to_owned(),
            icon: n.icon.to_owned(),
        })
        .collect();
    Ok((active.iter().map(|m| m.id.to_owned()).collect(), nav))
}

// ---------------------------------------------------------------------------------------------
// Spärr för modulernas anrop under en enhet

#[derive(Debug, Clone, Copy)]
struct ModuleId(&'static str);

async fn body_path(state: &AppState, key: &str) -> ApiResult<Option<String>> {
    let query = if let Ok(id) = key.parse::<Uuid>() {
        sqlx::query_scalar("SELECT path::text FROM body WHERE id = $1").bind(id)
    } else {
        let Ok(BodyKey { kind, number }) = key.parse() else { return Ok(None) };
        sqlx::query_scalar("SELECT path::text FROM body WHERE kind = $1 AND number = $2")
            .bind(kind.as_str())
            .bind(number)
    };
    Ok(query.fetch_optional(&state.db).await?)
}

/// Stoppar anrop till en modul som är av för enheten. Den som inte är inloggad eller inte får se
/// enheten får samma svar som för en enhet som inte finns.
async fn gate(
    State(state): State<AppState>,
    Extension(ModuleId(module)): Extension<ModuleId>,
    matched: MatchedPath,
    req: Request,
    next: Next,
) -> Response {
    let key = matched
        .as_str()
        .starts_with("/v1/bodies/{id}/")
        .then(|| req.uri().path().split('/').nth(3).map(str::to_owned))
        .flatten();
    let Some(key) = key else { return next.run(req).await };
    let path = match body_path(&state, &key).await {
        Ok(Some(path)) => path,
        Ok(None) => return next.run(req).await,
        Err(e) => return e.into_response(),
    };
    match is_active(&state, module, &path).await {
        Ok(true) => next.run(req).await,
        Ok(false) => {
            let (mut parts, _) = req.into_parts();
            match CurrentUser::from_request_parts(&mut parts, &state).await {
                Ok(user) if user.0.can(Permission::BodyRead, &path) => {
                    disabled(&state, module, &path).await.into_response()
                }
                Ok(_) => ApiError::NotFound.into_response(),
                Err(e) => e.into_response(),
            }
        }
        Err(e) => e.into_response(),
    }
}

/// Modulernas anrop (med spärr) och anropen för att välja moduler.
pub fn router(state: &AppState) -> Router<AppState> {
    let core = Router::new()
        .route("/v1/modules", get(list_catalog))
        .route("/v1/bodies/{id}/modules", get(get_body_modules))
        .route("/v1/bodies/{id}/modules/{module}", put(set_module));
    MODULES.iter().fold(core, |r, m| {
        r.merge(
            m.router()
                .route_layer(middleware::from_fn_with_state(state.clone(), gate))
                .route_layer(Extension(ModuleId(m.manifest().id))),
        )
    })
}

#[derive(Debug, OpenApi)]
#[openapi(
    paths(list_catalog, get_body_modules, set_module),
    tags((name = "modules", description = "Modulkatalog och val av moduler per förbund, distrikt och kår"))
)]
struct ModulesApi;

/// `OpenAPI` för modulkatalogen och alla inbyggda moduler.
pub fn openapi() -> utoipa::openapi::OpenApi {
    MODULES.iter().fold(ModulesApi::openapi(), |doc, m| doc.merge_from(m.openapi()))
}

// ---------------------------------------------------------------------------------------------
// API: katalog och val per enhet

#[derive(Debug, Serialize, ToSchema)]
pub struct ModuleNav {
    pub segment: String,
    pub label: String,
    pub kinds: Vec<String>,
    pub permission: Permission,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ModuleInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub availability: Availability,
    pub default_enabled: bool,
    pub depends_on: Vec<String>,
    pub nav: Vec<ModuleNav>,
}

impl From<&Manifest> for ModuleInfo {
    fn from(m: &Manifest) -> Self {
        Self {
            id: m.id.into(),
            name: m.name.into(),
            description: m.description.into(),
            icon: m.icon.into(),
            availability: m.availability,
            default_enabled: m.default_enabled,
            depends_on: m.depends_on.iter().map(|d| (*d).into()).collect(),
            nav: m
                .nav
                .iter()
                .map(|n| ModuleNav {
                    segment: n.segment.into(),
                    label: n.label.into(),
                    kinds: n.kinds.iter().map(|k| (*k).into()).collect(),
                    permission: n.permission,
                })
                .collect(),
        }
    }
}

/// Alla moduler: inbyggda och planerade.
#[utoipa::path(get, path = "/v1/modules", tag = "modules",
    responses((status = 200, body = [ModuleInfo]), (status = 401, body = crate::error::Problem)))]
pub async fn list_catalog(_user: CurrentUser) -> Json<Vec<ModuleInfo>> {
    Json(catalog().map(ModuleInfo::from).collect())
}

/// En moduls läge för en enhet.
#[derive(Debug, Serialize, ToSchema)]
#[expect(clippy::struct_excessive_bools, reason = "API-svar med oberoende flaggor för gränssnittet")]
pub struct BodyModule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub availability: Availability,
    /// Namnen på moduler som krävs.
    pub depends_on: Vec<String>,
    /// På för enheten, efter arv, låsning och beroenden.
    pub enabled: bool,
    /// Valet utan hänsyn till beroenden.
    pub chosen: bool,
    /// `default` (standardläge), `inherited` (från en överliggande enhet) eller `own` (valt här).
    pub source: String,
    /// Enheten som valet ärvs från.
    pub source_body: Option<String>,
    /// Överliggande enhet som har låst valet.
    pub locked_by: Option<String>,
    /// Valet är låst här för underliggande enheter.
    pub locked_here: bool,
    /// Namnen på moduler som krävs men inte är på.
    pub missing: Vec<String>,
    /// Om den inloggade får ändra valet här.
    pub can_change: bool,
    /// Om valet kan låsas här (förbund och distrikt).
    pub can_lock: bool,
}

async fn body_modules(
    state: &AppState,
    user: &CurrentUser,
    body_id: Uuid,
    kind: &str,
    path: &str,
) -> ApiResult<Vec<BodyModule>> {
    let resolved = resolve(&load_settings(&state.db, path).await?, body_id);
    let manage = is_settable(kind) && user.0.can(Permission::ModulesManage, path);
    Ok(catalog()
        .map(|m| {
            let r = &resolved[m.id];
            let available = m.availability == Availability::Available;
            BodyModule {
                id: m.id.into(),
                name: m.name.into(),
                description: m.description.into(),
                icon: m.icon.into(),
                availability: m.availability,
                depends_on: m.depends_on.iter().map(|d| name_of(d).into()).collect(),
                enabled: r.enabled,
                chosen: r.chosen,
                source: match r.source {
                    Source::Default => "default",
                    Source::Inherited(_) => "inherited",
                    Source::Own => "own",
                }
                .into(),
                source_body: match &r.source {
                    Source::Inherited(b) => Some(b.clone()),
                    _ => None,
                },
                locked_by: r.locked_by.clone(),
                locked_here: r.locked_here,
                missing: r.missing.iter().map(|d| name_of(d).into()).collect(),
                can_change: manage && available && r.locked_by.is_none(),
                can_lock: manage && available && r.locked_by.is_none() && kind != "group",
            }
        })
        .collect())
}

/// Modulernas läge för en enhet, med varifrån valet kommer.
#[utoipa::path(get, path = "/v1/bodies/{id}/modules", tag = "modules",
    params(("id" = String, Path, description = "Enhetens id eller nyckel, t.ex. `group-1001`")),
    responses((status = 200, body = [BodyModule]), (status = 404, body = crate::error::Problem)))]
pub async fn get_body_modules(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(key): Path<String>,
) -> ApiResult<Json<Vec<BodyModule>>> {
    let (id, kind, path) = load_visible_body(&state, &user, &key, Permission::BodyRead).await?;
    Ok(Json(body_modules(&state, &user, id, &kind, &path).await?))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetModule {
    /// `true` eller `false`. `null` tar bort enhetens eget val, så att valet ärvs igen.
    pub enabled: Option<bool>,
    /// Lås valet för underliggande enheter (förbund och distrikt).
    #[serde(default)]
    pub locked: bool,
}

fn join_names(ids: &[&str]) -> String {
    ids.iter().map(|d| name_of(d)).collect::<Vec<_>>().join(" och ")
}

/// Slår på eller av en modul för en enhet, eller återgår till det ärvda valet. Kräver `modules_manage`.
#[utoipa::path(put, path = "/v1/bodies/{id}/modules/{module}", tag = "modules",
    params(("id" = String, Path, description = "Enhetens id eller nyckel"), ("module" = String, Path, description = "Modulens id, t.ex. `grants`")),
    request_body = SetModule,
    responses((status = 200, body = [BodyModule]), (status = 400, body = crate::error::Problem), (status = 403, body = crate::error::Problem),
        (status = 404, body = crate::error::Problem), (status = 409, description = "Låst av en överliggande enhet eller i konflikt med ett beroende", body = crate::error::Problem)))]
pub async fn set_module(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((key, module)): Path<(String, String)>,
    Json(req): Json<SetModule>,
) -> ApiResult<Json<Vec<BodyModule>>> {
    let (body_id, kind, path) = load_visible_body(&state, &user, &key, Permission::BodyRead).await?;
    if !user.0.can(Permission::ModulesManage, &path) {
        return Err(ApiError::Forbidden);
    }
    if !is_settable(&kind) {
        return Err(ApiError::BadRequest("moduler väljs för en kår, ett distrikt eller förbundet".into()));
    }
    let m = manifest(&module).ok_or(ApiError::NotFound)?;
    if m.availability != Availability::Available {
        return Err(ApiError::Conflict(format!("{} är planerad och finns inte ännu", m.name)));
    }
    if req.locked && (kind == "group" || req.enabled.is_none()) {
        return Err(ApiError::BadRequest(
            "bara förbund och distrikt kan låsa ett val, och då måste det vara på eller av".into(),
        ));
    }

    let mut settings = load_settings(&state.db, &path).await?;
    let before = resolve(&settings, body_id);
    if let Some(by) = &before[m.id].locked_by {
        return Err(ApiError::Conflict(format!("{} är låst av {by}", m.name)));
    }
    // Pröva ändringen innan den sparas, så att beroenden inte bryts.
    let body_name = sqlx::query_scalar::<_, String>("SELECT name FROM body WHERE id = $1")
        .bind(body_id)
        .fetch_one(&state.db)
        .await?;
    settings.retain(|s| !(s.body_id == body_id && s.module_id == m.id));
    if let Some(enabled) = req.enabled {
        settings.push(Setting { module_id: m.id.into(), enabled, locked: req.locked, body_id, body_name });
    }
    let after = resolve(&settings, body_id);
    if req.enabled == Some(true) && !after[m.id].enabled {
        return Err(ApiError::Conflict(format!(
            "{} kräver {}. Slå på det först.",
            m.name,
            join_names(&after[m.id].missing)
        )));
    }
    let broken: Vec<&str> =
        catalog().filter(|x| x.id != m.id && before[x.id].enabled && !after[x.id].enabled).map(|x| x.id).collect();
    if !broken.is_empty() {
        let names = join_names(&broken);
        return Err(ApiError::Conflict(format!("{names} kräver {}. Stäng av {names} först.", m.name)));
    }

    match req.enabled {
        Some(enabled) => {
            sqlx::query(
                "INSERT INTO module_setting (body_id, module_id, enabled, locked, updated_by) VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT (body_id, module_id)
                 DO UPDATE SET enabled = EXCLUDED.enabled, locked = EXCLUDED.locked, updated_by = EXCLUDED.updated_by, updated_at = now()",
            )
            .bind(body_id)
            .bind(m.id)
            .bind(enabled)
            .bind(req.locked)
            .bind(user.0.account_id)
            .execute(&state.db)
            .await?;
        }
        None => {
            sqlx::query("DELETE FROM module_setting WHERE body_id = $1 AND module_id = $2")
                .bind(body_id)
                .bind(m.id)
                .execute(&state.db)
                .await?;
        }
    }
    audit(
        &state.db,
        user.0.account_id,
        "module.update",
        "body",
        body_id,
        serde_json::json!({ "module": m.id, "enabled": req.enabled, "locked": req.locked }),
    )
    .await;
    Ok(Json(body_modules(&state, &user, body_id, &kind, &path).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setting(module: &str, enabled: bool, locked: bool, body: u128, name: &str) -> Setting {
        Setting { module_id: module.into(), enabled, locked, body_id: Uuid::from_u128(body), body_name: name.into() }
    }

    #[test]
    fn dependencies_come_first_and_exist() {
        let ids: Vec<&str> = catalog().map(|m| m.id).collect();
        for (i, m) in catalog().enumerate() {
            for d in m.depends_on {
                let pos = ids.iter().position(|x| x == d).expect("beroendet finns i katalogen");
                assert!(pos < i, "{} måste stå före {}", d, m.id);
            }
        }
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), ids.len(), "unika id");
    }

    #[test]
    fn defaults_apply_without_settings() {
        let r = resolve(&[], Uuid::nil());
        assert!(r["attendance"].enabled && r["grants"].enabled);
        assert_eq!(r["attendance"].source, Source::Default);
        assert!(!r["billing"].enabled, "planerade moduler är av");
    }

    #[test]
    fn nearest_choice_wins_and_dependencies_switch_off() {
        let kar = 3;
        let settings =
            [setting("attendance", false, false, 2, "Distriktet"), setting("grants", true, false, kar, "Kåren")];
        let r = resolve(&settings, Uuid::from_u128(kar));
        assert!(!r["attendance"].enabled);
        assert_eq!(r["attendance"].source, Source::Inherited("Distriktet".into()));
        assert!(r["grants"].chosen && !r["grants"].enabled, "bidrag kräver närvaro");
        assert_eq!(r["grants"].missing, vec!["attendance"]);
        assert_eq!(r["grants"].source, Source::Own);
    }

    #[test]
    fn lock_above_overrides_own_choice() {
        let kar = 3;
        let settings = [
            setting("statistics", false, true, 1, "Förbundet"),
            setting("statistics", true, false, 2, "Distriktet"),
            setting("statistics", true, false, kar, "Kåren"),
        ];
        let r = resolve(&settings, Uuid::from_u128(kar));
        assert!(!r["statistics"].enabled);
        assert_eq!(r["statistics"].locked_by.as_deref(), Some("Förbundet"));
        let own = resolve(&settings[..1], Uuid::from_u128(1));
        assert!(own["statistics"].locked_here && own["statistics"].locked_by.is_none());
    }
}
