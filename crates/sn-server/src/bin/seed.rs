//! Fyller databasen med syntetisk demodata.
//!
//! `cargo run -p sn-server --bin seed -- [--districts N] [--groups N] [--seed N]`

use anyhow::Context;
use sn_server::{
    MIGRATOR,
    seed::{SeedConfig, run},
};
use sqlx::postgres::PgPoolOptions;

fn arg(args: &[String], name: &str) -> Option<usize> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).and_then(|v| v.parse().ok())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut cfg = SeedConfig::default();
    if let Some(n) = arg(&args, "--districts") {
        cfg.districts = n;
    }
    if let Some(n) = arg(&args, "--groups") {
        cfg.groups_per_district = n;
    }
    if let Some(n) = arg(&args, "--seed") {
        cfg.seed = n as u64;
    }

    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://scoutnet2:scoutnet2-dev-only@127.0.0.1:5434/scoutnet2".into());
    let db =
        PgPoolOptions::new().max_connections(4).connect(&url).await.context("kunde inte ansluta till databasen")?;
    MIGRATOR.run(&db).await?;

    let started = std::time::Instant::now();
    let s = run(&db, &cfg).await?;
    println!(
        "Klart på {:.1} s: {} enheter, {} personer, {} medlemskap, {} möten, {} närvaroposter",
        started.elapsed().as_secs_f64(),
        s.bodies,
        s.persons,
        s.memberships,
        s.meetings,
        s.attendance
    );
    println!("Personas för snabbinloggning:");
    for (persona, id) in s.personas {
        println!("  {persona}  ({id})");
    }
    Ok(())
}
