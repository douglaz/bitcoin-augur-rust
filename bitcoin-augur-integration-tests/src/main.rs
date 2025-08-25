mod api;
mod cli;
mod comparison;
mod parity;
mod report;
mod server;
mod tests;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::cli::{Cli, Commands, StartServerArgs};
use crate::server::{KotlinServer, RustServer, Server};

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("bitcoin_augur_integration_tests={log_level}").into()),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .compact(),
        )
        .init();

    // Execute command
    match cli.command {
        Commands::Test(args) => {
            tests::run_integration_tests(args).await?;
        }
        Commands::Parity(args) => {
            parity::run_parity_tests(args).await?;
        }
        Commands::BuildKotlin => {
            build_kotlin_jar().await?;
        }
        Commands::Validate => {
            validate_environment().await?;
        }
        Commands::StartRust(args) => {
            start_rust_server(args).await?;
        }
        Commands::StartKotlin(args) => {
            start_kotlin_server(args).await?;
        }
    }

    Ok(())
}

async fn validate_environment() -> Result<()> {
    use colored::*;

    println!("{}", "Validating environment...".bold());

    // Check for Rust server binary
    let rust_server = which::which("bitcoin-augur-server").or_else(|_| {
        // Try to find it in target directory
        let paths = vec![
            "target/release/bitcoin-augur-server",
            "target/debug/bitcoin-augur-server",
            "../target/release/bitcoin-augur-server",
            "../target/debug/bitcoin-augur-server",
            "target/x86_64-unknown-linux-musl/release/bitcoin-augur-server",
            "target/x86_64-unknown-linux-musl/debug/bitcoin-augur-server",
            "target/x86_64-unknown-linux-gnu/release/bitcoin-augur-server",
            "target/x86_64-unknown-linux-gnu/debug/bitcoin-augur-server",
        ];
        for path in paths {
            if std::path::Path::new(path).exists() {
                return Ok(std::path::PathBuf::from(path));
            }
        }
        Err(which::Error::CannotFindBinaryPath)
    });

    match rust_server {
        Ok(path) => println!(
            "✅ Rust server found: {}",
            path.display().to_string().green()
        ),
        Err(_) => {
            let cmd = "cargo build --release -p bitcoin-augur-server".yellow();
            println!("❌ Rust server not found. Run: {cmd}");
        }
    }

    // Check for Java
    match which::which("java") {
        Ok(path) => {
            let java_path = path.display().to_string().green();
            println!("✅ Java found: {java_path}");
        }
        Err(_) => println!("❌ Java not found. Please install Java 17 or later"),
    }

    // Check for Kotlin JAR
    let kotlin_jar = std::path::Path::new("../bitcoin-augur-reference/app/build/libs/app-all.jar");
    if kotlin_jar.exists() {
        let jar_path = kotlin_jar.display().to_string().green();
        println!("✅ Kotlin reference JAR found: {jar_path}");
    } else {
        let cmd = "./bitcoin-augur-integration-tests build-kotlin".yellow();
        println!("❌ Kotlin JAR not found. Run: {cmd}");

        // Check for gradle
        match which::which("gradle") {
            Ok(path) => {
                let gradle_path = path.display().to_string().green();
                println!("   Gradle available at: {gradle_path}");
            }
            Err(_) => {
                println!("   ⚠️ Gradle not found. Use nix develop or install Gradle");
            }
        }
    }

    // Check Bitcoin Core connectivity (optional)
    let msg = "Bitcoin Core connectivity will be tested when running tests".italic();
    println!("\n{msg}");

    Ok(())
}

async fn build_kotlin_jar() -> Result<()> {
    use colored::*;

    let msg = "Building Kotlin reference JAR...".bold();
    println!("{msg}");

    // Check if gradle is available
    let gradle_check = tokio::process::Command::new("gradle")
        .arg("--version")
        .output()
        .await;

    if gradle_check.is_err() {
        println!(
            "{}",
            "❌ Gradle not found. Please install Gradle or use nix develop".red()
        );
        return Err(anyhow::anyhow!("Gradle not available"));
    }

    // Check if Java is available
    let java_check = tokio::process::Command::new("java")
        .arg("-version")
        .output()
        .await;

    if java_check.is_err() {
        let msg = "❌ Java not found. Please install Java 17 or use nix develop".red();
        println!("{msg}");
        return Err(anyhow::anyhow!("Java not available"));
    }

    // Navigate to the Kotlin reference directory
    let kotlin_ref_dir = std::path::Path::new("../bitcoin-augur-reference");
    if !kotlin_ref_dir.exists() {
        let msg = "❌ bitcoin-augur-reference directory not found".red();
        println!("{msg}");
        println!("  Expected at: ../bitcoin-augur-reference");
        return Err(anyhow::anyhow!("Kotlin reference not found"));
    }

    let path = kotlin_ref_dir.display();
    println!("📁 Found Kotlin reference at: {path}");

    // Get Java home
    let java_home = std::env::var("JAVA_HOME").ok().or_else(|| {
        // Try to infer JAVA_HOME from java binary
        which::which("java").ok().and_then(|java_path| {
            java_path
                .parent()?
                .parent()
                .map(|p| p.to_string_lossy().to_string())
        })
    });

    // Build the shadowJar
    println!("🔨 Running gradle shadowJar...");
    let mut cmd = tokio::process::Command::new("gradle");

    if let Some(java_home) = java_home {
        cmd.arg(format!("-Dorg.gradle.java.home={java_home}"));
    }

    cmd.arg("shadowJar")
        .current_dir(kotlin_ref_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let output = cmd.output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let msg = "❌ Build failed:".red();
        println!("{msg}");
        println!("{stderr}");
        return Err(anyhow::anyhow!("Gradle build failed"));
    }

    // Check if JAR was created
    let jar_path = kotlin_ref_dir.join("app/build/libs/app-all.jar");
    if jar_path.exists() {
        let metadata = std::fs::metadata(&jar_path)?;
        let size_mb = metadata.len() as f64 / 1_048_576.0;
        let path = jar_path.display();
        let msg = format!("✅ JAR built successfully: {path} ({size_mb:.2} MB)").green();
        println!("{msg}");
    } else {
        let msg = "❌ JAR not found after build".red();
        println!("{msg}");
        return Err(anyhow::anyhow!("JAR not created"));
    }

    Ok(())
}

async fn start_rust_server(args: StartServerArgs) -> Result<()> {
    use colored::*;
    use std::time::Duration;

    let title = "Starting Rust Bitcoin Augur Server".bold().cyan();
    println!("{title}");
    println!("{}", "=================================".cyan());

    // Start mock RPC if requested
    let _mock_rpc = if args.use_mock_rpc {
        let mock_port = args.mock_rpc_port;
        let msg = format!("Starting mock Bitcoin RPC on port {mock_port}").yellow();
        println!("{msg}");

        let mock = std::sync::Arc::new(parity::MockBitcoinRpc::new(mock_port));
        let mock_clone = mock.clone();

        tokio::spawn(async move {
            if let Err(e) = mock_clone.start().await {
                tracing::error!("Mock RPC server error: {e}");
            }
        });

        // Give mock RPC time to start
        tokio::time::sleep(Duration::from_millis(500)).await;
        Some(mock)
    } else {
        None
    };

    // Configure Bitcoin RPC URL
    let bitcoin_rpc = if args.use_mock_rpc {
        let mock_port = args.mock_rpc_port;
        format!("http://127.0.0.1:{mock_port}")
    } else {
        args.bitcoin_rpc.clone()
    };

    // Pre-populate data if we're using mock RPC and want to init from store
    let _temp_dir_handle; // Keep temp_dir alive
    let data_dir = if args.use_mock_rpc && args.init_from_store {
        let temp_dir = tempfile::TempDir::new()?;
        let data_path = temp_dir.path().join("mempool_data");

        let msg = "Pre-populating snapshot data...".yellow();
        println!("{msg}");
        parity::snapshot_generator::generate_and_save_snapshots(&data_path, 24)?;

        let count_msg = format!("Generated 144 snapshots in {}", data_path.display()).green();
        println!("{count_msg}");

        _temp_dir_handle = temp_dir; // Move ownership to keep it alive
        Some(data_path)
    } else {
        _temp_dir_handle = tempfile::TempDir::new()?; // Create dummy to satisfy type
        None
    };

    // Create and start the Rust server
    let mut server = RustServer::new(
        args.port,
        args.binary,
        bitcoin_rpc.clone(),
        args.rpc_user.clone(),
        args.rpc_password.clone(),
    )?;

    // Set pre-populated data directory if available
    if let Some(data_dir) = data_dir {
        server.set_data_directory(data_dir);
    }

    let port = args.port;
    let msg = format!("Starting server on port {port}...").green();
    println!("{msg}");
    server.start().await?;

    // Wait for server to be ready
    println!("Waiting for server to be ready...");
    server.wait_for_ready(Duration::from_secs(30)).await?;

    let ready_msg = format!("✅ Server is running at http://127.0.0.1:{port}")
        .green()
        .bold();
    println!("{ready_msg}");
    println!();
    let endpoints = "Available endpoints:".bold();
    println!("{endpoints}");
    let health_url = format!("  - http://127.0.0.1:{port}/health");
    let fees_url = format!("  - http://127.0.0.1:{port}/fees");
    println!("{health_url}");
    println!("{fees_url}");
    println!();
    let stop_msg = "Press Ctrl+C to stop the server".italic();
    println!("{stop_msg}");

    // Keep the server running
    tokio::signal::ctrl_c().await?;

    println!("\n{}", "Stopping server...".yellow());
    server.stop().await?;

    Ok(())
}

async fn start_kotlin_server(args: StartServerArgs) -> Result<()> {
    use colored::*;
    use std::time::Duration;

    let title = "Starting Kotlin Bitcoin Augur Server".bold().cyan();
    println!("{title}");
    println!("{}", "====================================".cyan());

    // Start mock RPC if requested
    let _mock_rpc = if args.use_mock_rpc {
        let mock_port = args.mock_rpc_port;
        let msg = format!("Starting mock Bitcoin RPC on port {mock_port}").yellow();
        println!("{msg}");

        let mock = std::sync::Arc::new(parity::MockBitcoinRpc::new(mock_port));
        let mock_clone = mock.clone();

        tokio::spawn(async move {
            if let Err(e) = mock_clone.start().await {
                tracing::error!("Mock RPC server error: {e}");
            }
        });

        // Give mock RPC time to start
        tokio::time::sleep(Duration::from_millis(500)).await;
        Some(mock)
    } else {
        None
    };

    // Configure Bitcoin RPC URL
    let bitcoin_rpc = if args.use_mock_rpc {
        let mock_port = args.mock_rpc_port;
        format!("http://127.0.0.1:{mock_port}")
    } else {
        args.bitcoin_rpc.clone()
    };

    // Create and start the Kotlin server
    let mut server = KotlinServer::new(
        args.port,
        args.binary,
        bitcoin_rpc.clone(),
        args.rpc_user.clone(),
        args.rpc_password.clone(),
    )?;

    let port = args.port;
    let msg = format!("Starting server on port {port}...").green();
    println!("{msg}");
    server.start().await?;

    // Wait for server to be ready
    println!("Waiting for server to be ready...");
    server.wait_for_ready(Duration::from_secs(30)).await?;

    let ready_msg = format!("✅ Server is running at http://127.0.0.1:{port}")
        .green()
        .bold();
    println!("{ready_msg}");
    println!();
    let endpoints = "Available endpoints:".bold();
    println!("{endpoints}");
    let fees_url = format!("  - http://127.0.0.1:{port}/fees");
    let historical_url =
        format!("  - http://127.0.0.1:{port}/historical_fee?timestamp=<unix_timestamp>");
    println!("{fees_url}");
    println!("{historical_url}");
    println!();
    let stop_msg = "Press Ctrl+C to stop the server".italic();
    println!("{stop_msg}");

    // Keep the server running
    tokio::signal::ctrl_c().await?;

    println!("\n{}", "Stopping server...".yellow());
    server.stop().await?;

    Ok(())
}
