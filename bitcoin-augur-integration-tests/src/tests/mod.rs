mod advanced;
mod basic;

use anyhow::Result;
use colored::*;
use std::time::Duration;
use tracing::{error, info};

use crate::cli::TestArgs;
use crate::report::TestReport;
use crate::server::{KotlinServer, RustServer, Server};

pub async fn run_integration_tests(args: TestArgs) -> Result<()> {
    println!("{}", "Bitcoin Augur Integration Tests".bold().cyan());
    println!("{}", "================================".cyan());

    let mut report = TestReport::new();

    // Initialize servers
    let mut rust_server: Option<RustServer> = None;
    let mut kotlin_server: Option<KotlinServer> = None;

    if !args.skip_rust {
        rust_server = Some(RustServer::new(
            args.rust_port,
            args.rust_binary.clone(),
            args.bitcoin_rpc.clone(),
            args.rpc_user.clone(),
            args.rpc_password.clone(),
        )?);
    }

    if !args.skip_kotlin {
        kotlin_server = Some(KotlinServer::new(
            args.kotlin_port,
            args.kotlin_jar.clone(),
            args.bitcoin_rpc.clone(),
            args.rpc_user.clone(),
            args.rpc_password.clone(),
        )?);
    }

    // Start servers
    let startup_timeout = Duration::from_secs(args.startup_timeout);

    if let Some(ref mut server) = rust_server {
        info!("Starting Rust server...");
        server.start().await?;
        server.wait_for_ready(startup_timeout).await?;
        report.rust_server_started = true;
    }

    if let Some(ref mut server) = kotlin_server {
        info!("Starting Kotlin server...");
        server.start().await?;
        server.wait_for_ready(startup_timeout).await?;
        report.kotlin_server_started = true;
    }

    // Wait a bit for servers to collect initial data
    info!("Waiting for servers to collect initial mempool data...");
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Run tests
    let test_result = run_tests(&args, &rust_server, &kotlin_server, &mut report).await;

    // Clean up
    if let Some(ref mut server) = rust_server {
        if let Err(e) = server.stop().await {
            error!("Failed to stop Rust server: {}", e);
        }
    }

    if let Some(ref mut server) = kotlin_server {
        if let Err(e) = server.stop().await {
            error!("Failed to stop Kotlin server: {}", e);
        }
    }

    // Print report
    report.print_summary();

    // Return error if tests failed
    test_result?;

    if !report.all_passed() {
        anyhow::bail!("Some tests failed");
    }

    Ok(())
}

async fn run_tests(
    args: &TestArgs,
    rust_server: &Option<RustServer>,
    kotlin_server: &Option<KotlinServer>,
    report: &mut TestReport,
) -> Result<()> {
    // If comparing both servers
    if rust_server.is_some() && kotlin_server.is_some() {
        let rust = rust_server.as_ref().unwrap();
        let kotlin = kotlin_server.as_ref().unwrap();

        // Run basic comparison tests
        basic::run_basic_comparison_tests(rust, kotlin, report).await?;

        // Run advanced tests if no specific test specified
        if args.test_name.is_none() {
            advanced::run_advanced_tests(rust, kotlin, report).await?;
        }
    }
    // If only testing one server
    else if let Some(rust) = rust_server {
        basic::run_single_server_tests(rust, report).await?;
    } else if let Some(kotlin) = kotlin_server {
        basic::run_single_server_tests(kotlin, report).await?;
    }

    Ok(())
}
