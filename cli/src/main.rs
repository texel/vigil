//! vigil-cli — command-line interface for the vigil task scheduler.

mod commands;

use anyhow::{Context, Result};
use clap::Parser;
use commands::Cli;
use std::path::PathBuf;
use vigil_registry::Store;

fn vigil_dir() -> PathBuf {
    dirs_path().join(".vigil")
}

fn dirs_path() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn db_path() -> PathBuf {
    vigil_dir().join("state.db")
}

fn logs_dir() -> PathBuf {
    vigil_dir().join("logs")
}

async fn ensure_vigil_dir() -> Result<()> {
    tokio::fs::create_dir_all(vigil_dir())
        .await
        .context("failed to create ~/.vigil directory")?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();
    ensure_vigil_dir().await?;
    let store = Store::open(&db_path()).await?;

    commands::dispatch(cli, store).await
}
