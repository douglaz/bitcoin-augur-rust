use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

mod api_client;
mod compatibility;
mod mock_rpc;
mod runner;
mod server;
mod snapshots;
mod stress;
mod test_cases;
mod test_vectors;

use runner::TestRunner;

/// Bitcoin Augur Regression Testing CLI
///
/// Tests API compatibility between Rust and reference implementations
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to bitcoin-augur-server binary (auto-detected if not specified)
    #[arg(long, env = "BITCOIN_AUGUR_SERVER_PATH")]
    server_path: Option<PathBuf>,

    /// Path to reference implementation JAR (for cross-implementation testing)
    #[arg(long, env = "BITCOIN_AUGUR_REFERENCE_JAR")]
    reference_jar: Option<PathBuf>,

    /// Server port (default: random available port)
    #[arg(long, short = 'p', env = "TEST_SERVER_PORT")]
    port: Option<u16>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info", env = "RUST_LOG")]
    log_level: String,

    /// Test data directory
    #[arg(long, default_value = "test-data", env = "TEST_DATA_DIR")]
    data_dir: PathBuf,

    /// Update snapshots instead of comparing
    #[arg(long)]
    update_snapshots: bool,

    /// Test filter pattern
    #[arg(long, short = 'f')]
    filter: Option<String>,

    /// Number of parallel tests
    #[arg(long, short = 'j', default_value = "4")]
    jobs: usize,

    /// Verbose output
    #[arg(long, short = 'v')]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run all regression tests
    Run {
        /// Skip snapshot tests
        #[arg(long)]
        skip_snapshots: bool,

        /// Skip compatibility tests
        #[arg(long)]
        skip_compatibility: bool,

        /// Skip test vectors
        #[arg(long)]
        skip_vectors: bool,
    },

    /// Run API compatibility tests only
    Compatibility {
        /// Test against reference implementation
        #[arg(long)]
        with_reference: bool,
    },

    /// Run snapshot tests only
    Snapshots {
        /// Force update all snapshots
        #[arg(long)]
        force_update: bool,
    },

    /// Run test vector validation only
    Vectors {
        /// Path to test vectors JSON file
        #[arg(long)]
        vectors_file: Option<PathBuf>,
    },

    /// Generate test data for regression testing
    Generate {
        /// Output directory for generated data
        #[arg(long, default_value = "generated-test-data")]
        output: PathBuf,

        /// Number of test cases to generate
        #[arg(long, default_value = "100")]
        count: usize,
    },

    /// Compare two API responses for compatibility
    Compare {
        /// First API endpoint URL
        endpoint1: String,

        /// Second API endpoint URL
        endpoint2: String,

        /// Request path to test
        #[arg(default_value = "/fees")]
        path: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&cli.log_level));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(cli.verbose)
        .with_thread_ids(cli.verbose)
        .with_line_number(cli.verbose)
        .with_writer(std::io::stderr) // Send tracing to stderr
        .init();

    // Create test runner
    let mut runner = TestRunner::new(
        cli.server_path,
        cli.reference_jar,
        cli.port,
        cli.data_dir,
        cli.update_snapshots,
        cli.filter,
        cli.jobs,
    )?;

    match cli.command {
        Commands::Run {
            skip_snapshots,
            skip_compatibility,
            skip_vectors,
        } => {
            runner
                .run_all(!skip_snapshots, !skip_compatibility, !skip_vectors)
                .await?;
        }
        Commands::Compatibility { with_reference } => {
            runner.run_compatibility_tests(with_reference).await?;
        }
        Commands::Snapshots { force_update } => {
            runner.run_snapshot_tests(force_update).await?;
        }
        Commands::Vectors { vectors_file } => {
            runner.run_vector_tests(vectors_file).await?;
        }
        Commands::Generate { output, count } => {
            runner.generate_test_data(output, count).await?;
        }
        Commands::Compare {
            endpoint1,
            endpoint2,
            path,
        } => {
            runner
                .compare_endpoints(&endpoint1, &endpoint2, &path)
                .await?;
        }
    }

    Ok(())
}
