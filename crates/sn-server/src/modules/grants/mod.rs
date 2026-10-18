//! Modul Bidrag: kommunala aktivitetsbidrag (stadsbidrag). Regler per kommun som data,
//! kontroll av varje sammankomst och DAK-export. Bygger på modulen Närvaro.

pub mod report;

use utoipa::OpenApi;

use super::{Availability, Manifest, Module, NavEntry};
use crate::{AppState, authz::Permission};

pub const ID: &str = "grants";

static MANIFEST: Manifest = Manifest {
    id: ID,
    name: "Bidrag",
    description: "Underlag för kommunens aktivitetsbidrag (stadsbidrag): varje sammankomst kontrolleras mot \
                  kommunens regler, bidraget räknas fram och närvaron exporteras som DAK-fil.",
    icon: "📄",
    availability: Availability::Available,
    default_enabled: true,
    depends_on: &[super::attendance::ID],
    nav: &[NavEntry {
        segment: "bidrag",
        label: "Bidrag",
        icon: "📄",
        kinds: &["group", "troop"],
        permission: Permission::ReportsRead,
    }],
};

#[derive(Debug, OpenApi)]
#[openapi(
    paths(report::grant_report, report::dak_export),
    tags((name = "grants", description = "Modul Bidrag: bidragsunderlag och DAK-export (kräver modulen Närvaro)"))
)]
struct Api;

#[derive(Debug)]
pub struct Grants;

impl Module for Grants {
    fn manifest(&self) -> &'static Manifest {
        &MANIFEST
    }

    fn router(&self) -> axum::Router<AppState> {
        report::router()
    }

    fn openapi(&self) -> utoipa::openapi::OpenApi {
        Api::openapi()
    }
}
