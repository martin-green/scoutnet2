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
            Muterande anrop kräver headern `x-sn-csrf: 1`.\n\n\
            Kärnan (organisation, medlemmar, personer) kompletteras av moduler som Närvaro, Bidrag och Statistik. \
            En modul kan vara av för en enhet, och då ger dess anrop 404 med felkoden `module_disabled`.",
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
        routes::views::list_views,
        routes::views::create_view,
        routes::views::delete_view,
        routes::bulk::bulk,
        routes::audit_log::person_audit,
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
        (name = "views", description = "Sparade vyer"),
        (name = "auth", description = "Inloggning och sessioner"),
    ),
    modifiers(&SessionCookie)
)]
pub struct ApiDoc;

/// Hela specifikationen: kärnan och alla inbyggda moduler.
pub fn spec() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi().merge_from(crate::modules::openapi())
}

#[derive(Debug)]
struct SessionCookie;

impl Modify for SessionCookie {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components
            .add_security_scheme("session", SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("sn_session"))));
    }
}
