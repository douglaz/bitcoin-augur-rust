mod helpers;
mod mock_rpc;
mod scenarios;
pub mod snapshot_generator;
mod test_data;

pub use mock_rpc::MockBitcoinRpc;

use crate::cli::ParityArgs;
use crate::report::TestReport;
use crate::server::{KotlinServer, RustServer, Server};
use anyhow::Result;
use std::time::Duration;
use tempfile::TempDir;
use tracing::info;

pub async fn run_parity_tests(args: ParityArgs) -> Result<()> {
    use colored::*;

    let title = "Bitcoin Augur Kotlin Parity Tests".bold().cyan();
    let separator = "==================================".cyan();
    println!("{title}");
    println!("{separator}");

    let mut report = TestReport::new();

    // Initialize servers
    let mut rust_server = RustServer::new(
        args.rust_port,
        args.rust_binary.clone(),
        args.bitcoin_rpc.clone(),
        args.rpc_user.clone(),
        args.rpc_password.clone(),
    )?;

    let mut kotlin_server = KotlinServer::new(
        args.kotlin_port,
        args.kotlin_jar.clone(),
        args.bitcoin_rpc.clone(),
        args.rpc_user.clone(),
        args.rpc_password.clone(),
    )?;

    // Start mock RPC if requested
    let mock_rpc = if args.use_mock_rpc {
        let mock_port = args.mock_rpc_port;
        info!("Starting mock Bitcoin RPC server on port {mock_port}");
        let mock = std::sync::Arc::new(MockBitcoinRpc::new(args.mock_rpc_port));

        // Start server in background using the same instance
        let mock_clone = mock.clone();
        tokio::spawn(async move {
            if let Err(e) = mock_clone.start().await {
                tracing::error!("Mock RPC server error: {e}");
            }
        });

        // Give mock RPC time to start
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Update server configs to use mock RPC
        let mock_port = args.mock_rpc_port;
        rust_server = RustServer::new(
            args.rust_port,
            args.rust_binary.clone(),
            format!("http://127.0.0.1:{mock_port}"),
            Some("mockuser".to_string()),
            Some("mockpass".to_string()),
        )?;

        let mock_port = args.mock_rpc_port;
        kotlin_server = KotlinServer::new(
            args.kotlin_port,
            args.kotlin_jar.clone(),
            format!("http://127.0.0.1:{mock_port}"),
            Some("mockuser".to_string()),
            Some("mockpass".to_string()),
        )?;

        Some(mock)
    } else {
        None
    };

    // Pre-populate data directories with snapshots for tests that need them (3-12)
    // Tests 1-2 should start with empty data to test edge cases
    let should_prepopulate = args.use_mock_rpc
        && match args.test_number {
            Some(1) | Some(2) => false, // Tests 1-2 need empty/minimal data
            _ => true,                  // Tests 3-12 and full suite need pre-populated data
        };

    let temp_dirs = if should_prepopulate {
        info!("Pre-populating snapshot data for servers...");

        // Create temporary directories for data
        let rust_temp = TempDir::new()?;
        let kotlin_temp = TempDir::new()?;

        let rust_data_dir = rust_temp.path().join("mempool_data");
        let kotlin_data_dir = kotlin_temp.path().join("mempool_data");

        // Generate and save 24 hours of snapshot data
        snapshot_generator::setup_test_data(&rust_data_dir, &kotlin_data_dir)?;

        // Configure servers to use the pre-populated data
        rust_server.set_data_directory(rust_data_dir);
        kotlin_server.set_data_directory(kotlin_data_dir);

        info!("Snapshot data pre-population complete");

        Some((rust_temp, kotlin_temp))
    } else {
        if args.use_mock_rpc {
            info!("Starting servers without pre-populated data (tests 1-2 require empty state)");
        }
        None
    };

    // Start servers
    let startup_timeout = Duration::from_secs(args.startup_timeout);

    info!("Starting Rust server...");
    rust_server.start().await?;
    rust_server.wait_for_ready(startup_timeout).await?;
    report.rust_server_started = true;

    info!("Starting Kotlin server...");
    kotlin_server.start().await?;
    kotlin_server.wait_for_ready(startup_timeout).await?;
    report.kotlin_server_started = true;

    // Wait for initial data collection
    info!("Waiting for servers to initialize...");
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Run parity tests
    let test_result = if let Some(test_num) = args.test_number {
        scenarios::run_single_parity_test(
            &rust_server,
            &kotlin_server,
            test_num,
            args.tolerance,
            &mut report,
            mock_rpc.as_deref(),
        )
        .await
    } else {
        scenarios::run_all_parity_tests(
            &rust_server,
            &kotlin_server,
            args.tolerance,
            &mut report,
            mock_rpc.as_deref(),
        )
        .await
    };

    // Clean up
    rust_server.stop().await?;
    kotlin_server.stop().await?;

    // Keep temp directories alive until here
    drop(temp_dirs);

    // Print report
    report.print_summary();

    test_result?;

    if !report.all_passed() {
        anyhow::bail!("Some parity tests failed");
    }

    let success_msg = "✅ Full Kotlin parity achieved!".bold().green();
    println!("\n{success_msg}");
    Ok(())
}
