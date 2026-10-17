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
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://scoutnet2:scoutnet2-dev-only@127.0.0.1:5433/scoutnet2".into());
        let bind = std::env::var("SN_BIND")
            .unwrap_or_else(|_| "127.0.0.1:8080".into())
            .parse()
            .context("SN_BIND måste vara host:port")?;
        let web_dist = std::env::var_os("SN_WEB_DIST").map(PathBuf::from).or_else(|| {
            let default = PathBuf::from("web/build");
            default.join("200.html").exists().then_some(default)
        });
        Ok(Self {
            database_url,
            bind,
            dev_login: env_flag("SN_DEV_LOGIN", true),
            cookie_secure: env_flag("SN_COOKIE_SECURE", false),
            web_dist,
            session_hours: std::env::var("SN_SESSION_HOURS").ok().and_then(|v| v.parse().ok()).unwrap_or(12),
        })
    }

    pub fn session_cookie_name(&self) -> &'static str {
        if self.cookie_secure { "__Host-sn_session" } else { "sn_session" }
    }
}

fn env_flag(name: &str, default: bool) -> bool {
    std::env::var(name).map_or(default, |v| matches!(v.as_str(), "1" | "true" | "yes" | "on"))
}
