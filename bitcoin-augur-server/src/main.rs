//! Bitcoin Augur Server - HTTP API for fee estimation service

mod api;
mod bitcoin;
mod config;
mod persistence;
mod server;
mod service;

use anyhow::{Context, Result};
use bitcoin_augur::FeeEstimator;
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    bitcoin::BitcoinRpcClient,
    config::AppConfig,
    persistence::SnapshotStore,
    server::{create_app, run_server},
    service::MempoolCollector,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing to stderr
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "bitcoin_augur_server=info,bitcoin_augur=info".into()),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_target(false)
                .compact(),
        )
        .init();

    info!("Bitcoin Augur Server starting...");

    // Load configuration
    let config = AppConfig::load().context("Failed to load configuration")?;

    info!("Configuration loaded:");
    info!("  Server: {}:{}", config.server.host, config.server.port);
    info!("  Bitcoin RPC: {}", config.bitcoin_rpc.url);
    info!("  Data directory: {}", config.persistence.data_directory);
    info!("  Collection interval: {}ms", config.collector.interval_ms);

    // Initialize Bitcoin RPC client
    let bitcoin_client = BitcoinRpcClient::new(config.to_bitcoin_rpc_config());

    // Test Bitcoin connection
    match bitcoin_client.test_connection().await {
        Ok(_) => info!("Successfully connected to Bitcoin Core"),
        Err(e) => {
            error!("Failed to connect to Bitcoin Core: {}", e);
            error!("Please ensure Bitcoin Core is running and RPC credentials are correct");
            // Continue anyway - the collector will retry
        }
    }

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

    // Spawn background collection task
    let collector_handle = collector.clone();
    let interval_ms = config.collector.interval_ms;
    tokio::spawn(async move {
        info!("Starting mempool collector with {}ms interval", interval_ms);
        if let Err(e) = collector_handle.start(interval_ms).await {
            error!("Mempool collector error: {}", e);
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
                Ok(deleted) => info!("Cleaned up {} old snapshot directories", deleted),
                Err(e) => error!("Cleanup failed: {}", e),
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
