mod error;
mod format;
mod parser;
mod security;
mod server;
mod storage;
mod summarizer;
mod tools;

use anyhow::Result;
use clap::Parser;
use rmcp::{ServiceExt, transport::stdio};
use tracing_subscriber::{self, EnvFilter};

use server::MunchServer;

#[derive(Parser, Debug)]
#[command(
    name = "munchrs",
    version,
    about = "MCP server for codebase indexing and symbol retrieval"
)]
struct Cli {
    /// Log level
    #[arg(long, default_value = "warn", env = "MUNCHRS_LOG_LEVEL")]
    log_level: String,

    /// Log file path (defaults to stderr)
    #[arg(long, env = "MUNCHRS_LOG_FILE")]
    log_file: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let filter = EnvFilter::try_new(&cli.log_level).unwrap_or_else(|_| EnvFilter::new("warn"));

    if let Some(log_file) = &cli.log_file {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)?;
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(file)
            .with_ansi(false)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .with_ansi(false)
            .init();
    }

    tracing::info!("Starting munchrs MCP server");

    let storage_path = std::env::var("CODE_INDEX_PATH").ok();
    let server = MunchServer::new(storage_path);

    let service = server.serve(stdio()).await.inspect_err(|e| {
        tracing::error!("serving error: {:?}", e);
    })?;

    service.waiting().await?;
    Ok(())
}
