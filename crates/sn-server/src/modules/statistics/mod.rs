//! Modul Statistik: medlemsutveckling, ålder och kön, ledartäthet, närvaro per gren och
//! väntelista för en kår, ett distrikt eller hela förbundet.
//!
//! Exempel på en ny modul: manifestet, ett anrop och en sida i webbappen. Den läser kärnans
//! data men ändrar ingenting, och närvaron tas bara med om modulen Närvaro är på.

pub mod report;

use utoipa::OpenApi;

use super::{Availability, Manifest, Module, NavEntry};
use crate::{AppState, authz::Permission};

pub const ID: &str = "statistics";

static MANIFEST: Manifest = Manifest {
    id: ID,
    name: "Statistik",
    description: "Medlemsutveckling termin för termin, ålder och kön, antal scouter per ledare, närvaro per gren \
                  och väntelistan – för kåren, distriktet eller hela förbundet.",
    icon: "📊",
    availability: Availability::Available,
    default_enabled: true,
    depends_on: &[],
    nav: &[NavEntry {
        segment: "statistik",
        label: "Statistik",
        icon: "📊",
        kinds: &["organisation", "region", "district", "group", "troop"],
        permission: Permission::ReportsRead,
    }],
};

#[derive(Debug, OpenApi)]
#[openapi(
    paths(report::statistics),
    tags((name = "statistics", description = "Modul Statistik: medlemsutveckling, ålder, ledartäthet och närvaro"))
)]
struct Api;

#[derive(Debug)]
pub struct Statistics;

impl Module for Statistics {
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
