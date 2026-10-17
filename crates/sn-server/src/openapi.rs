use utoipa::{
    Modify, OpenApi,
    openapi::security::{ApiKey, ApiKeyValue, SecurityScheme},
};

use crate::{auth, routes};

#[derive(Debug, OpenApi)]
#[openapi(
    info(
        title = "Scoutnet 2 API (prototyp)",
        version = "0.1.0",
        description = "API-först: allt som webbappen gör finns här. All data i prototypen är syntetisk.\n\n\
            Autentisering sker med sessionscookie (BFF) som skapas via ScoutID, eller med snabbinloggning i demoläge.\n\
            Muterande anrop kräver headern `x-sn-csrf: 1`.",
        license(name = "MIT", url = "https://opensource.org/licenses/MIT")
    ),
    paths(
        routes::me::me,
        routes::bodies::get_body,
        routes::bodies::get_children,
        routes::bodies::get_descendants,
        routes::members::list_members,
        routes::people::get_person,
        routes::people::reveal_national_id,
        routes::attendance::attendance_summary,
        routes::attendance::attendance_matrix,
        routes::attendance::create_meetings,
        routes::attendance::get_meeting,
        routes::attendance::update_meeting,
        routes::attendance::delete_meeting,
        routes::attendance::put_attendance,
        routes::views::list_views,
        routes::views::create_view,
        routes::views::delete_view,
        routes::grants::grant_report,
        routes::grants::dak_export,
        auth::dev_personas,
        auth::dev_login,
        auth::logout,
        crate::oidc::auth_config,
        crate::oidc::login,
        crate::oidc::callback,
    ),
    tags(
        (name = "me", description = "Inloggad användare"),
        (name = "organisation", description = "Organisationsträdet"),
        (name = "members", description = "Medlemmar och personer"),
        (name = "attendance", description = "Möten och närvaro"),
        (name = "views", description = "Sparade vyer"),
        (name = "grants", description = "Bidragsunderlag och DAK"),
        (name = "auth", description = "Inloggning och sessioner"),
    ),
    modifiers(&SessionCookie)
)]
pub struct ApiDoc;

#[derive(Debug)]
struct SessionCookie;

impl Modify for SessionCookie {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components
            .add_security_scheme("session", SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("sn_session"))));
    }
}
