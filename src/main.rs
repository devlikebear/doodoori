use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod cli;
mod claude;
mod config;
mod executor;
mod git;
mod hooks;
mod instructions;
mod loop_engine;
mod notifications;
mod pricing;
mod sandbox;
mod secrets;
mod state;
mod utils;
mod watch;
mod workflow;

use cli::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if present (before anything else)
    if let Err(e) = dotenvy::dotenv() {
        // Only warn if the file exists but couldn't be loaded
        if std::path::Path::new(".env").exists() {
            eprintln!("Warning: Failed to load .env file: {}", e);
        }
    }

    // Initialize tracing
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Parse CLI arguments
    let cli = Cli::parse();

    // Run the CLI
    cli.run().await
}
