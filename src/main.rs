use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod cli;
mod claude;
mod config;
mod executor;
mod instructions;
mod loop_engine;
mod pricing;
mod sandbox;
mod state;
mod utils;

use cli::Cli;

#[tokio::main]
async fn main() -> Result<()> {
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
