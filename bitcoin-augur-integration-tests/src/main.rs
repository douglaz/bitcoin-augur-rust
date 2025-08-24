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

use crate::cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("bitcoin_augur_integration_tests={}", log_level).into()
            }),
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
        Err(_) => println!(
            "❌ Rust server not found. Run: {}",
            "cargo build --release -p bitcoin-augur-server".yellow()
        ),
    }

    // Check for Java
    match which::which("java") {
        Ok(path) => println!("✅ Java found: {}", path.display().to_string().green()),
        Err(_) => println!("❌ Java not found. Please install Java 17 or later"),
    }

    // Check for Kotlin JAR
    let kotlin_jar = std::path::Path::new("../bitcoin-augur-reference/app/build/libs/app-all.jar");
    if kotlin_jar.exists() {
        println!(
            "✅ Kotlin reference JAR found: {}",
            kotlin_jar.display().to_string().green()
        );
    } else {
        println!(
            "❌ Kotlin JAR not found. Run: {}",
            "./bitcoin-augur-integration-tests build-kotlin".yellow()
        );

        // Check for gradle
        match which::which("gradle") {
            Ok(path) => {
                println!(
                    "   Gradle available at: {}",
                    path.display().to_string().green()
                );
            }
            Err(_) => {
                println!("   ⚠️ Gradle not found. Use nix develop or install Gradle");
            }
        }
    }

    // Check Bitcoin Core connectivity (optional)
    println!(
        "\n{}",
        "Bitcoin Core connectivity will be tested when running tests".italic()
    );

    Ok(())
}

async fn build_kotlin_jar() -> Result<()> {
    use colored::*;

    println!("{}", "Building Kotlin reference JAR...".bold());

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
        println!(
            "{}",
            "❌ Java not found. Please install Java 17 or use nix develop".red()
        );
        return Err(anyhow::anyhow!("Java not available"));
    }

    // Navigate to the Kotlin reference directory
    let kotlin_ref_dir = std::path::Path::new("../bitcoin-augur-reference");
    if !kotlin_ref_dir.exists() {
        println!("{}", "❌ bitcoin-augur-reference directory not found".red());
        println!("  Expected at: ../bitcoin-augur-reference");
        return Err(anyhow::anyhow!("Kotlin reference not found"));
    }

    println!("📁 Found Kotlin reference at: {}", kotlin_ref_dir.display());

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
        cmd.arg(format!("-Dorg.gradle.java.home={}", java_home));
    }

    cmd.arg("shadowJar")
        .current_dir(kotlin_ref_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let output = cmd.output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("{}", "❌ Build failed:".red());
        println!("{}", stderr);
        return Err(anyhow::anyhow!("Gradle build failed"));
    }

    // Check if JAR was created
    let jar_path = kotlin_ref_dir.join("app/build/libs/app-all.jar");
    if jar_path.exists() {
        let metadata = std::fs::metadata(&jar_path)?;
        let size_mb = metadata.len() as f64 / 1_048_576.0;
        println!(
            "{}",
            format!(
                "✅ JAR built successfully: {} ({:.2} MB)",
                jar_path.display(),
                size_mb
            )
            .green()
        );
    } else {
        println!("{}", "❌ JAR not found after build".red());
        return Err(anyhow::anyhow!("JAR not created"));
    }

    Ok(())
}
