//! Bitcoin Augur Server - HTTP API for fee estimation service

mod api;
mod bitcoin;
mod cli;
mod config;
mod persistence;
mod server;
mod service;

use anyhow::{Context, Result};
use bitcoin_augur::FeeEstimator;
use clap::Parser;
use std::sync::Arc;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    bitcoin::{BitcoinClient, BitcoinRpcClient, MockBitcoinClient},
    cli::Cli,
    config::AppConfig,
    persistence::SnapshotStore,
    server::{create_app, run_server},
    service::MempoolCollector,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize tracing to stderr with CLI-provided filter
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from(cli.log_filter.as_str())
                .context("Invalid log filter")?,
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_target(false)
                .compact(),
        )
        .init();

    info!("Bitcoin Augur Server starting...");

    // Load configuration with CLI overrides
    let config = AppConfig::load_with_cli(&cli).context("Failed to load configuration")?;

    info!("Configuration loaded:");
    info!(
        "  Server: {host}:{port}",
        host = config.server.host,
        port = config.server.port
    );
    info!("  Bitcoin RPC: {url}", url = config.bitcoin_rpc.url);
    info!(
        "  Data directory: {dir}",
        dir = config.persistence.data_directory
    );
    info!(
        "  Collection interval: {interval}ms",
        interval = config.collector.interval_ms
    );
    info!("  Test mode: {enabled}", enabled = config.test_mode.enabled);

    // Initialize Bitcoin RPC client (use mock if in test mode)
    let bitcoin_client = if config.test_mode.enabled {
        info!("Running in test mode - using mock Bitcoin client");
        BitcoinClient::Mock(MockBitcoinClient::new())
    } else {
        let client = BitcoinRpcClient::new(config.to_bitcoin_rpc_config());

        // Test Bitcoin connection
        match client.test_connection().await {
            Ok(_) => info!("Successfully connected to Bitcoin Core"),
            Err(e) => {
                error!("Failed to connect to Bitcoin Core: {e}");
                error!("Please ensure Bitcoin Core is running and RPC credentials are correct");
                // Continue anyway - the collector will retry
            }
        }

        BitcoinClient::Real(client)
    };

    // Initialize persistence store
    let snapshot_store = SnapshotStore::new(&config.persistence.data_directory)
        .context("Failed to initialize snapshot store")?;

    // Initialize fee estimator
    let fee_estimator = FeeEstimator::new();

    // Create mempool collector
    let collector = Arc::new(MempoolCollector::new(
        bitcoin_client,
        snapshot_store,
        fee_estimator,
    ));

    // Initialize from stored snapshots if requested
    if cli.init_from_store {
        info!("Initializing fee estimates from stored snapshots...");
        match collector.initialize_from_store().await {
            Ok(_) => info!("Successfully initialized estimates from stored snapshots"),
            Err(e) => warn!("Failed to initialize from store: {e}"),
        }
    }

    // Spawn background collection task
    let collector_handle = collector.clone();
    let interval_ms = config.collector.interval_ms;
    tokio::spawn(async move {
        info!("Starting mempool collector with {interval_ms}ms interval");
        if let Err(e) = collector_handle.start(interval_ms).await {
            error!("Mempool collector error: {e}");
        }
    });

    // Spawn periodic cleanup task (runs daily)
    let collector_cleanup = collector.clone();
    let cleanup_days = config.persistence.cleanup_days;
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(24 * 60 * 60));
        loop {
            interval.tick().await;
            info!(
                "Running snapshot cleanup (keeping last {} days)",
                cleanup_days
            );
            match collector_cleanup.cleanup_old_snapshots(cleanup_days).await {
                Ok(deleted) => info!("Cleaned up {deleted} old snapshot directories"),
                Err(e) => error!("Cleanup failed: {e}"),
            }
        }
    });

    // Create and run HTTP server
    let app = create_app(collector);

    run_server(app, config.server.host, config.server.port)
        .await
        .context("Failed to run HTTP server")?;

    info!("Bitcoin Augur Server shut down");

    Ok(())
}
