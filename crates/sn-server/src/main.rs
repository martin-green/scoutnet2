use anyhow::Context;
use sn_server::{AppState, Config, MIGRATOR, app};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "sn_server=info,tower_http=info".into()))
        .init();

    let config = Config::from_env()?;
    if config.dev_login {
        tracing::warn!("SN_DEV_LOGIN är på: snabbinloggning med personas. Får aldrig användas i produktion.");
    }
    // Samma fråga kan gälla en patrull eller hela förbundet. sqlx återanvänder förberedda frågor,
    // och en generisk plan för hierarkifilter blir mycket fel för stora enheter. Planera därför
    // varje körning med de verkliga parametrarna (kostar 1–5 ms).
    let options = config
        .database_url
        .parse::<PgConnectOptions>()
        .context("ogiltig DATABASE_URL")?
        .options([("plan_cache_mode", "force_custom_plan")]);
    let db = PgPoolOptions::new()
        .max_connections(20)
        .connect_with(options)
        .await
        .context("kunde inte ansluta till databasen")?;
    MIGRATOR.run(&db).await.context("migrering misslyckades")?;

    let bind = config.bind;
    let state = AppState::new(db, config)?;
    let listener = TcpListener::bind(bind).await?;
    tracing::info!("Scoutnet 2 lyssnar på http://{bind}  (API-dokumentation: http://{bind}/docs)");
    axum::serve(listener, app(&state)).with_graceful_shutdown(shutdown()).await?;
    Ok(())
}

async fn shutdown() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("stänger ned");
}
