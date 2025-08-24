mod helpers;
mod mock_rpc;
mod scenarios;
mod test_data;

pub use mock_rpc::MockBitcoinRpc;

use crate::cli::ParityArgs;
use crate::report::TestReport;
use crate::server::{KotlinServer, RustServer, Server};
use anyhow::Result;
use std::time::Duration;
use tracing::info;

pub async fn run_parity_tests(args: ParityArgs) -> Result<()> {
    use colored::*;

    println!("{}", "Bitcoin Augur Kotlin Parity Tests".bold().cyan());
    println!("{}", "==================================".cyan());

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
    let _mock_rpc = if args.use_mock_rpc {
        info!(
            "Starting mock Bitcoin RPC server on port {}",
            args.mock_rpc_port
        );
        let mock = std::sync::Arc::new(MockBitcoinRpc::new(args.mock_rpc_port));

        // Start server in background
        let mock_port = args.mock_rpc_port;
        tokio::spawn(async move {
            let mock_server = MockBitcoinRpc::new(mock_port);
            if let Err(e) = mock_server.start().await {
                tracing::error!("Mock RPC server error: {}", e);
            }
        });

        // Give mock RPC time to start
        tokio::time::sleep(Duration::from_secs(1)).await;

        // Update server configs to use mock RPC
        rust_server = RustServer::new(
            args.rust_port,
            args.rust_binary.clone(),
            format!("http://127.0.0.1:{}", args.mock_rpc_port),
            Some("mockuser".to_string()),
            Some("mockpass".to_string()),
        )?;

        kotlin_server = KotlinServer::new(
            args.kotlin_port,
            args.kotlin_jar.clone(),
            format!("http://127.0.0.1:{}", args.mock_rpc_port),
            Some("mockuser".to_string()),
            Some("mockpass".to_string()),
        )?;

        Some(mock)
    } else {
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
        )
        .await
    } else {
        scenarios::run_all_parity_tests(&rust_server, &kotlin_server, args.tolerance, &mut report)
            .await
    };

    // Clean up
    rust_server.stop().await?;
    kotlin_server.stop().await?;

    // Print report
    report.print_summary();

    test_result?;

    if !report.all_passed() {
        anyhow::bail!("Some parity tests failed");
    }

    println!("\n{}", "✅ Full Kotlin parity achieved!".bold().green());
    Ok(())
}
