//! Inloggning via `ScoutID` (Keycloak) med OIDC authorization code + PKCE enligt BFF-mönstret:
//! tokens stannar på servern och webbläsaren får bara en `HttpOnly`-sessionscookie (planen 7.2).
//!
//! Kontot kopplas via ScoutID:s `sub`, och första gången via medlemsnumret (`scoutnet_member_no`,
//! eller `preferred_username` på formen `<nr>@scoutnet`).

use std::{
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::{DateTime, Utc};
use openidconnect::{
    AdditionalClaims, AdditionalProviderMetadata, AuthenticationFlow, AuthorizationCode, ClientId, ClientSecret,
    CsrfToken, IdToken, IssuerUrl, Nonce, PkceCodeChallenge, PkceCodeVerifier, ProviderMetadata, RedirectUrl, Scope,
    core::{
        CoreAuthDisplay, CoreClaimName, CoreClaimType, CoreClient, CoreClientAuthMethod, CoreGenderClaim,
        CoreGrantType, CoreJsonWebKey, CoreJweContentEncryptionAlgorithm, CoreJweKeyManagementAlgorithm,
        CoreJwsSigningAlgorithm, CoreResponseMode, CoreResponseType, CoreSubjectIdentifierType,
    },
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    AppState,
    auth::{create_session, hash_token, session_cookie},
    config::OidcConfig,
    error::{ApiError, ApiResult},
};

const STATE_COOKIE: &str = "sn_oidc";
const LOGIN_TTL_MINUTES: i64 = 10;
const DISCOVERY_RETRY: Duration = Duration::from_secs(15);

/// `end_session_endpoint` finns inte i kärnmetadatan och läses därför som tillägg.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogoutMetadata {
    end_session_endpoint: Option<String>,
}
impl AdditionalProviderMetadata for LogoutMetadata {}

type Metadata = ProviderMetadata<
    LogoutMetadata,
    CoreAuthDisplay,
    CoreClientAuthMethod,
    CoreClaimName,
    CoreClaimType,
    CoreGrantType,
    CoreJweContentEncryptionAlgorithm,
    CoreJweKeyManagementAlgorithm,
    CoreJsonWebKey,
    CoreResponseMode,
    CoreResponseType,
    CoreSubjectIdentifierType,
>;

/// ScoutID:s egna claims i ID-tokenet.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct ScoutIdClaims {
    /// Sträng eller tal beroende på mapper.
    scoutnet_member_no: Option<serde_json::Value>,
}
impl AdditionalClaims for ScoutIdClaims {}

type ScoutIdToken = IdToken<ScoutIdClaims, CoreGenderClaim, CoreJweContentEncryptionAlgorithm, CoreJwsSigningAlgorithm>;

/// OIDC-klient med cachad discovery (metadata och nycklar).
pub struct Oidc {
    config: OidcConfig,
    redirect_uri: String,
    post_logout_uri: String,
    http: reqwest::Client,
    metadata: tokio::sync::Mutex<(Option<Arc<Metadata>>, Option<Instant>)>,
}

impl std::fmt::Debug for Oidc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Oidc").field("config", &self.config).finish_non_exhaustive()
    }
}

impl Oidc {
    pub fn new(config: OidcConfig, public_url: &str) -> anyhow::Result<Self> {
        let http = reqwest::ClientBuilder::new()
            // Att följa omdirigeringar öppnar för SSRF.
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(5))
            .build()?;
        Ok(Self {
            config,
            redirect_uri: format!("{public_url}/auth/callback"),
            post_logout_uri: format!("{public_url}/login"),
            http,
            metadata: tokio::sync::Mutex::new((None, None)),
        })
    }

    /// Hämtar discovery-dokument och nycklar första gången. Misslyckade försök görs om tidigast efter 15 s.
    async fn metadata(&self) -> Option<Arc<Metadata>> {
        let mut guard = self.metadata.lock().await;
        if let Some(m) = &guard.0 {
            return Some(m.clone());
        }
        if guard.1.is_some_and(|t| t.elapsed() < DISCOVERY_RETRY) {
            return None;
        }
        let issuer = IssuerUrl::new(self.config.issuer.clone()).ok()?;
        match Metadata::discover_async(issuer, &self.http).await {
            Ok(m) => {
                let m = Arc::new(m);
                *guard = (Some(m.clone()), None);
                tracing::info!(issuer = %self.config.issuer, "ScoutID (OIDC) är tillgängligt");
                Some(m)
            }
            Err(e) => {
                tracing::warn!(issuer = %self.config.issuer, error = %e, "ScoutID (OIDC) svarar inte");
                guard.1 = Some(Instant::now());
                None
            }
        }
    }

    fn client(
        &self,
        metadata: &Metadata,
    ) -> ApiResult<
        CoreClient<
            openidconnect::EndpointSet,
            openidconnect::EndpointNotSet,
            openidconnect::EndpointNotSet,
            openidconnect::EndpointNotSet,
            openidconnect::EndpointMaybeSet,
            openidconnect::EndpointMaybeSet,
        >,
    > {
        let redirect = RedirectUrl::new(self.redirect_uri.clone()).map_err(|e| anyhow::anyhow!("redirect-URI: {e}"))?;
        Ok(CoreClient::from_provider_metadata(
            metadata.clone(),
            ClientId::new(self.config.client_id.clone()),
            Some(ClientSecret::new(self.config.client_secret.clone())),
        )
        .set_redirect_uri(redirect))
    }

    /// Utloggnings-URL hos ScoutID (RP-initierad utloggning), om leverantören stöder det.
    pub async fn logout_url(&self, id_token: &str) -> Option<String> {
        let metadata = self.metadata().await?;
        let end = metadata.additional_metadata().end_session_endpoint.as_deref()?;
        let mut url = reqwest::Url::parse(end).ok()?;
        url.query_pairs_mut()
            .append_pair("id_token_hint", id_token)
            .append_pair("post_logout_redirect_uri", &self.post_logout_uri)
            .append_pair("client_id", &self.config.client_id);
        Some(url.into())
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthConfig {
    /// Snabbinloggning med personas (endast utveckling och demo).
    pub dev_login: bool,
    /// Inloggning via ScoutID är konfigurerad och svarar.
    pub scoutid: bool,
}

/// Vilka inloggningssätt som finns. Används av inloggningssidan.
#[utoipa::path(get, path = "/auth/config", tag = "auth", responses((status = 200, body = AuthConfig)))]
pub async fn auth_config(State(state): State<AppState>) -> Json<AuthConfig> {
    let scoutid = match &state.oidc {
        Some(o) => o.metadata().await.is_some(),
        None => false,
    };
    Json(AuthConfig { dev_login: state.config.dev_login, scoutid })
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct LoginQuery {
    /// Sida att återvända till efter inloggning (relativ sökväg).
    pub return_to: Option<String>,
}

/// Bara relativa sökvägar inom appen, så att inloggningen inte kan användas som öppen omdirigering.
fn safe_return_to(r: Option<&str>) -> String {
    match r {
        Some(p) if p.starts_with('/') && !p.starts_with("//") && !p.contains('\\') && !p.starts_with("/auth/") => {
            p.to_owned()
        }
        _ => "/".to_owned(),
    }
}

fn login_error(code: &str) -> Response {
    Redirect::to(&format!("/login?fel={code}")).into_response()
}

/// Startar inloggning via ScoutID: skapar state, nonce och PKCE och skickar webbläsaren till ScoutID.
#[utoipa::path(get, path = "/auth/login", tag = "auth", params(LoginQuery), responses((status = 303, description = "Till ScoutID")))]
pub async fn login(State(state): State<AppState>, jar: CookieJar, Query(q): Query<LoginQuery>) -> ApiResult<Response> {
    let oidc = state.oidc.as_ref().ok_or(ApiError::NotFound)?;
    let Some(metadata) = oidc.metadata().await else {
        return Ok(login_error("scoutid_nere"));
    };
    let client = oidc.client(&metadata)?;
    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let (url, csrf, nonce) = client
        .authorize_url(
            AuthenticationFlow::<CoreResponseType>::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scope(Scope::new("profile".into()))
        .add_scope(Scope::new("email".into()))
        .set_pkce_challenge(challenge)
        .url();

    sqlx::query("DELETE FROM oidc_login WHERE created_at < now() - make_interval(mins => $1)")
        .bind(i32::try_from(LOGIN_TTL_MINUTES).unwrap_or(10))
        .execute(&state.db)
        .await?;
    sqlx::query("INSERT INTO oidc_login (state_hash, nonce, pkce_verifier, return_to) VALUES ($1, $2, $3, $4)")
        .bind(hash_token(csrf.secret()))
        .bind(nonce.secret())
        .bind(verifier.secret())
        .bind(safe_return_to(q.return_to.as_deref()))
        .execute(&state.db)
        .await?;

    // Lax räcker: återkomsten från ScoutID är en navigering på toppnivå med GET.
    let cookie = Cookie::build((STATE_COOKIE, csrf.secret().clone()))
        .http_only(true)
        .secure(state.config.cookie_secure)
        .same_site(SameSite::Lax)
        .path("/auth/callback")
        .max_age(time::Duration::minutes(LOGIN_TTL_MINUTES))
        .build();
    Ok((jar.add(cookie), Redirect::to(url.as_str())).into_response())
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

#[derive(sqlx::FromRow)]
struct PendingLogin {
    nonce: String,
    pkce_verifier: String,
    return_to: String,
}

/// Medlemsnummer ur ScoutID:s claims.
fn member_no(claims: &openidconnect::IdTokenClaims<ScoutIdClaims, CoreGenderClaim>) -> Option<i32> {
    let from_claim = match &claims.additional_claims().scoutnet_member_no {
        Some(serde_json::Value::String(s)) => s.trim().parse().ok(),
        Some(serde_json::Value::Number(n)) => n.as_i64().and_then(|n| i32::try_from(n).ok()),
        _ => None,
    };
    from_claim.or_else(|| {
        let user = claims.preferred_username()?.as_str();
        let digits = user.split(['@', '|']).find(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))?;
        digits.parse().ok()
    })
}

/// Återkomst från ScoutID: verifierar state, växlar koden mot tokens, verifierar ID-tokenet,
/// kopplar kontot och skapar en session.
#[utoipa::path(get, path = "/auth/callback", tag = "auth", params(CallbackQuery), responses((status = 303, description = "Till appen eller inloggningssidan med felkod")))]
pub async fn callback(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<CallbackQuery>,
) -> ApiResult<Response> {
    let oidc = state.oidc.as_ref().ok_or(ApiError::NotFound)?;
    // Läs state-cookien innan den tas bort. Den gäller bara den här inloggningen.
    let cookie_state = jar.get(STATE_COOKIE).map(|c| c.value().to_owned());
    let jar = jar.remove(Cookie::build(STATE_COOKIE).path("/auth/callback"));
    if q.error.is_some() {
        return Ok((jar, login_error("avbruten")).into_response());
    }
    let (Some(code), Some(returned_state)) = (q.code, q.state) else {
        return Ok((jar, login_error("ogiltig")).into_response());
    };
    // Webbläsaren som startade inloggningen måste vara samma som kommer tillbaka (skydd mot login-CSRF).
    if cookie_state.as_deref() != Some(returned_state.as_str()) {
        return Ok((jar, login_error("ogiltig")).into_response());
    }
    let Some(pending) = sqlx::query_as::<_, PendingLogin>(
        "DELETE FROM oidc_login WHERE state_hash = $1 AND created_at > now() - make_interval(mins => $2)
         RETURNING nonce, pkce_verifier, return_to",
    )
    .bind(hash_token(&returned_state))
    .bind(i32::try_from(LOGIN_TTL_MINUTES).unwrap_or(10))
    .fetch_optional(&state.db)
    .await?
    else {
        return Ok((jar, login_error("ogiltig")).into_response());
    };

    let Some(metadata) = oidc.metadata().await else {
        return Ok((jar, login_error("scoutid_nere")).into_response());
    };
    let client = oidc.client(&metadata)?;
    let token = match client
        .exchange_code(AuthorizationCode::new(code))
        .map_err(|e| anyhow::anyhow!("token-endpoint saknas: {e}"))?
        .set_pkce_verifier(PkceCodeVerifier::new(pending.pkce_verifier))
        .request_async(&oidc.http)
        .await
    {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "kodväxling mot ScoutID misslyckades");
            return Ok((jar, login_error("teknik")).into_response());
        }
    };
    let Some(raw_id_token) = token.extra_fields().id_token().map(ToString::to_string) else {
        return Ok((jar, login_error("teknik")).into_response());
    };
    let verifier = client.id_token_verifier();
    let nonce = Nonce::new(pending.nonce);
    let claims = match ScoutIdToken::from_str(&raw_id_token)
        .map_err(|e| e.to_string())
        .and_then(|t| t.into_claims(&verifier, &nonce).map_err(|e| e.to_string()))
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "ID-token från ScoutID kunde inte verifieras");
            return Ok((jar, login_error("teknik")).into_response());
        }
    };

    let subject = claims.subject().as_str().to_owned();
    let Some(account_id) = link_account(&state, &subject, member_no(&claims)).await? else {
        tracing::info!(sub = %subject, "ScoutID-konto utan medlem i Scoutnet 2");
        return Ok((jar, login_error("okand")).into_response());
    };
    let acr = claims.auth_context_ref().map_or_else(|| "1".to_owned(), |a| a.as_str().to_owned());
    let auth_time: Option<DateTime<Utc>> = claims.auth_time();
    let session =
        create_session(&state, account_id, &format!("scoutid:acr={acr}"), auth_time, Some(&raw_id_token)).await?;
    crate::routes::audit(
        &state.db,
        account_id,
        "auth.login",
        "account",
        account_id,
        serde_json::json!({ "method": "scoutid", "acr": acr }),
    )
    .await;
    Ok((jar.add(session_cookie(&state, session)), Redirect::to(&pending.return_to)).into_response())
}

/// Kopplar ScoutID-identiteten till ett konto: först via `sub`, annars via medlemsnumret (och sparar då `sub`).
/// Personer som är medlemmar men saknar konto får ett vid första inloggningen.
async fn link_account(state: &AppState, subject: &str, member_no: Option<i32>) -> ApiResult<Option<Uuid>> {
    if let Some((id,)) = sqlx::query_as::<_, (Uuid,)>("SELECT id FROM account WHERE idp_subject = $1")
        .bind(subject)
        .fetch_optional(&state.db)
        .await?
    {
        return Ok(Some(id));
    }
    let Some(member_no) = member_no else { return Ok(None) };
    let Some((person_id,)) = sqlx::query_as::<_, (Uuid,)>("SELECT id FROM person WHERE member_no = $1")
        .bind(member_no)
        .fetch_optional(&state.db)
        .await?
    else {
        return Ok(None);
    };
    // Ett konto som redan är kopplat till en annan ScoutID-identitet tas inte över.
    let id: Option<(Uuid,)> = sqlx::query_as(
        "INSERT INTO account (person_id, idp_subject) VALUES ($1, $2)
         ON CONFLICT (person_id) DO UPDATE SET idp_subject = EXCLUDED.idp_subject
           WHERE account.idp_subject IS NULL
         RETURNING id",
    )
    .bind(person_id)
    .bind(subject)
    .fetch_optional(&state.db)
    .await?;
    Ok(id.map(|(id,)| id))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/config", get(auth_config))
        .route("/auth/login", get(login))
        .route("/auth/callback", get(callback))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn return_to_is_relative_only() {
        assert_eq!(safe_return_to(Some("/kar/1001?x=1")), "/kar/1001?x=1");
        for bad in ["https://evil.example", "//evil.example", "/\\evil", "/auth/logout", "kar"] {
            assert_eq!(safe_return_to(Some(bad)), "/", "{bad}");
        }
        assert_eq!(safe_return_to(None), "/");
    }
}
