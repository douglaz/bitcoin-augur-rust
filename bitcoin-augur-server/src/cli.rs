//! Command-line interface configuration

use anyhow::{Context, Result};
use clap::Parser;

/// Bitcoin Augur Server CLI
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    // Server options
    /// Host to bind the server to
    #[arg(short = 'H', long, default_value = "0.0.0.0")]
    pub host: String,

    /// Port to listen on
    #[arg(short, long, default_value_t = 8080)]
    pub port: u16,

    // Bitcoin RPC options
    /// Bitcoin Core RPC URL
    #[arg(long, default_value = "http://localhost:8332")]
    pub rpc_url: String,

    /// Bitcoin Core RPC username
    #[arg(long)]
    pub rpc_username: Option<String>,

    /// Bitcoin Core RPC password
    #[arg(long)]
    pub rpc_password: Option<String>,

    /// Path to Bitcoin Core cookie file (alternative to username/password)
    #[arg(long)]
    pub rpc_cookie_file: Option<String>,

    // Data persistence
    /// Directory for storing mempool snapshots
    #[arg(short, long, default_value = "mempool_data")]
    pub data_dir: String,

    /// Days to keep old snapshots
    #[arg(long, default_value_t = 30)]
    pub cleanup_days: i64,

    // Collection settings
    /// Mempool collection interval in seconds
    #[arg(long, default_value_t = 30)]
    pub interval_secs: u64,

    // Test mode
    /// Enable test mode with mock Bitcoin client
    #[arg(long)]
    pub test_mode: bool,

    /// Use mock data in test mode
    #[arg(long)]
    pub use_mock_data: bool,

    // Logging
    /// Log filter (e.g., "bitcoin_augur_server=debug,bitcoin_augur=info")
    #[arg(long, default_value = "bitcoin_augur_server=info,bitcoin_augur=info")]
    pub log_filter: String,

    // Existing options
    /// Initialize fee estimates from stored snapshots on startup
    #[arg(long)]
    pub init_from_store: bool,

    /// Path to configuration file (overridden by CLI args)
    #[arg(short, long)]
    pub config: Option<String>,
}

/// Read Bitcoin Core cookie file and extract credentials
pub fn read_cookie_file(path: &str) -> Result<(String, String)> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read cookie file: {path}"))?;
    let parts: Vec<&str> = contents.trim().split(':').collect();
    if parts.len() != 2 {
        anyhow::bail!("Invalid cookie file format (expected username:password)");
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}
