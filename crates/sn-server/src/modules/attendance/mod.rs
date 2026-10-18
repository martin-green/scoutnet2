//! Modul Närvaro: terminsplanering, närvaroregistrering och kalenderprenumeration.
//!
//! Andra moduler (t.ex. Bidrag) använder terminshjälparna som exporteras här.

pub mod calendar;
pub mod meetings;

pub use meetings::{TermQuery, parse_term};
use utoipa::OpenApi;

use super::{Availability, Manifest, Module, NavEntry};
use crate::{AppState, authz::Permission};

pub const ID: &str = "attendance";

static MANIFEST: Manifest = Manifest {
    id: ID,
    name: "Närvaro",
    description: "Planera terminens möten, registrera närvaro i mobilen och prenumerera på mötena i kalendern. \
                  Ger närvaro per person i medlemslistan och underlag för bidrag.",
    icon: "✅",
    availability: Availability::Available,
    default_enabled: true,
    depends_on: &[],
    nav: &[NavEntry {
        segment: "narvaro",
        label: "Närvaro",
        icon: "✅",
        kinds: &["organisation", "region", "district", "group", "troop"],
        permission: Permission::AttendanceRead,
    }],
};

#[derive(Debug, OpenApi)]
#[openapi(
    paths(
        meetings::attendance_summary,
        meetings::attendance_matrix,
        meetings::create_meetings,
        meetings::get_meeting,
        meetings::update_meeting,
        meetings::delete_meeting,
        meetings::put_attendance,
        calendar::create_calendar,
        calendar::delete_calendar,
    ),
    tags((name = "attendance", description = "Modul Närvaro: möten, närvaro och kalenderprenumeration"))
)]
struct Api;

#[derive(Debug)]
pub struct Attendance;

impl Module for Attendance {
    fn manifest(&self) -> &'static Manifest {
        &MANIFEST
    }

    fn router(&self) -> axum::Router<AppState> {
        meetings::router().merge(calendar::router())
    }

    fn openapi(&self) -> utoipa::openapi::OpenApi {
        Api::openapi()
    }
}
