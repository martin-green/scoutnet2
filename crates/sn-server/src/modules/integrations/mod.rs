//! Modul Integrationer: API-nycklar per kår och Scoutnet-kompatibelt API för befintliga
//! integrationer (t.ex. Skojjt).

pub mod compat;

use utoipa::OpenApi;

use super::{Availability, Manifest, Module, NavEntry};
use crate::{AppState, authz::Permission};

pub const ID: &str = "integrations";

static MANIFEST: Manifest = Manifest {
    id: ID,
    name: "Integrationer",
    description: "API-nycklar för kårens egna system, med behörigheter för kontaktuppgifter och personnummer var för sig. \
                  Medlemslistan finns i samma format som dagens Scoutnet, så att t.ex. Skojjt kan flytta utan omskrivning.",
    icon: "🔌",
    availability: Availability::Available,
    default_enabled: true,
    depends_on: &[],
    nav: &[NavEntry {
        segment: "integrationer",
        label: "Integrationer",
        icon: "🔌",
        kinds: &["group"],
        permission: Permission::MembersManage,
    }],
};

#[derive(Debug, OpenApi)]
#[openapi(
    paths(compat::list_keys, compat::create_key, compat::revoke_key, compat::memberlist),
    tags((name = "integrations", description = "Modul Integrationer: API-nycklar och Scoutnet-kompatibelt API"))
)]
struct Api;

#[derive(Debug)]
pub struct Integrations;

impl Module for Integrations {
    fn manifest(&self) -> &'static Manifest {
        &MANIFEST
    }

    fn router(&self) -> axum::Router<AppState> {
        compat::router()
    }

    fn openapi(&self) -> utoipa::openapi::OpenApi {
        Api::openapi()
    }
}
