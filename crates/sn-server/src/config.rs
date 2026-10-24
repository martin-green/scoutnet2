use std::{net::SocketAddr, path::PathBuf};

use anyhow::Context;

/// Konfiguration från miljövariabler. Inga hemligheter i kod eller repo.
#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub bind: SocketAddr,
    /// Snabbinloggning med personas. Endast för utveckling och demo, aldrig i produktion.
    pub dev_login: bool,
    /// `Secure` och `__Host-`-prefix på sessionscookien (kräver https).
    pub cookie_secure: bool,
    /// Katalog med byggd webbapp (SPA) som serveras på `/`.
    pub web_dist: Option<PathBuf>,
    pub session_hours: i64,
    /// Publik bas-URL, används för OIDC-redirect. Standard `http://{bind}`.
    pub public_url: String,
    /// Inloggning via `ScoutID` (OIDC). `None` stänger av den.
    pub oidc: Option<OidcConfig>,
}

#[derive(Clone)]
pub struct OidcConfig {
    /// T.ex. `https://id.scouterna.se/realms/scoutid`.
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
}

// Hemligheten får aldrig hamna i loggar.
impl std::fmt::Debug for OidcConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OidcConfig")
            .field("issuer", &self.issuer)
            .field("client_id", &self.client_id)
            .finish_non_exhaustive()
    }
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://scoutnet2:scoutnet2-dev-only@127.0.0.1:5434/scoutnet2".into());
        let bind = std::env::var("SN_BIND")
            .unwrap_or_else(|_| "127.0.0.1:8090".into())
            .parse()
            .context("SN_BIND måste vara host:port")?;
        let web_dist = std::env::var_os("SN_WEB_DIST").map(PathBuf::from).or_else(|| {
            let default = PathBuf::from("web/build");
            default.join("200.html").exists().then_some(default)
        });
        let dev_login = env_flag("SN_DEV_LOGIN", true);
        // I utvecklingsläge pekar ScoutID som standard på den lokala Keycloaken i docker-compose.
        // `SN_OIDC_ISSUER=` (tomt) stänger av OIDC.
        let oidc = match std::env::var("SN_OIDC_ISSUER") {
            Ok(issuer) if issuer.trim().is_empty() => None,
            Ok(issuer) => Some(OidcConfig {
                issuer,
                client_id: std::env::var("SN_OIDC_CLIENT_ID").context("SN_OIDC_CLIENT_ID saknas")?,
                client_secret: std::env::var("SN_OIDC_CLIENT_SECRET").context("SN_OIDC_CLIENT_SECRET saknas")?,
            }),
            Err(_) if dev_login => Some(OidcConfig {
                issuer: "http://127.0.0.1:8180/realms/scoutid".into(),
                client_id: "scoutnet2-web".into(),
                client_secret: "scoutnet2-dev-secret".into(),
            }),
            Err(_) => None,
        };
        Ok(Self {
            database_url,
            bind,
            dev_login,
            cookie_secure: env_flag("SN_COOKIE_SECURE", false),
            web_dist,
            session_hours: std::env::var("SN_SESSION_HOURS").ok().and_then(|v| v.parse().ok()).unwrap_or(12),
            public_url: std::env::var("SN_PUBLIC_URL")
                .unwrap_or_else(|_| format!("http://{bind}"))
                .trim_end_matches('/')
                .to_owned(),
            oidc,
        })
    }

    pub fn session_cookie_name(&self) -> &'static str {
        if self.cookie_secure { "__Host-sn_session" } else { "sn_session" }
    }
}

fn env_flag(name: &str, default: bool) -> bool {
    std::env::var(name).map_or(default, |v| matches!(v.as_str(), "1" | "true" | "yes" | "on"))
}
