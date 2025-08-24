use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "bitcoin-augur-integration-tests")]
#[command(about = "Integration test suite for Bitcoin Augur implementations")]
#[command(version)]
pub struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Output results as JSON
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run integration tests comparing both servers
    Test(TestArgs),

    /// Run Kotlin parity tests
    Parity(ParityArgs),

    /// Build the Kotlin reference JAR
    BuildKotlin,

    /// Validate environment (check for required binaries)
    Validate,
}

#[derive(Parser)]
pub struct TestArgs {
    /// Port for Rust server
    #[arg(long, default_value = "8180")]
    pub rust_port: u16,

    /// Port for Kotlin/Java server
    #[arg(long, default_value = "8181")]
    pub kotlin_port: u16,

    /// Bitcoin RPC URL
    #[arg(long, default_value = "http://localhost:8332")]
    pub bitcoin_rpc: String,

    /// Bitcoin RPC username
    #[arg(long, env = "BITCOIN_RPC_USER")]
    pub rpc_user: Option<String>,

    /// Bitcoin RPC password
    #[arg(long, env = "BITCOIN_RPC_PASSWORD")]
    pub rpc_password: Option<String>,

    /// Path to test mempool data (optional)
    #[arg(long)]
    pub test_data: Option<String>,

    /// Rust server binary path (if not in PATH)
    #[arg(long)]
    pub rust_binary: Option<String>,

    /// Kotlin server JAR path
    #[arg(long)]
    pub kotlin_jar: Option<String>,

    /// Skip Rust server tests
    #[arg(long)]
    pub skip_rust: bool,

    /// Skip Kotlin server tests
    #[arg(long)]
    pub skip_kotlin: bool,

    /// Timeout for server startup in seconds
    #[arg(long, default_value = "30")]
    pub startup_timeout: u64,

    /// Specific test to run (if not specified, runs all)
    #[arg(long)]
    pub test_name: Option<String>,
}

#[derive(Parser)]
pub struct ParityArgs {
    /// Port for Rust server
    #[arg(long, default_value = "8180")]
    pub rust_port: u16,

    /// Port for Kotlin/Java server
    #[arg(long, default_value = "8181")]
    pub kotlin_port: u16,

    /// Bitcoin RPC URL
    #[arg(long, default_value = "http://localhost:8332")]
    pub bitcoin_rpc: String,

    /// Bitcoin RPC username
    #[arg(long, env = "BITCOIN_RPC_USER")]
    pub rpc_user: Option<String>,

    /// Bitcoin RPC password
    #[arg(long, env = "BITCOIN_RPC_PASSWORD")]
    pub rpc_password: Option<String>,

    /// Rust server binary path (if not in PATH)
    #[arg(long)]
    pub rust_binary: Option<String>,

    /// Kotlin server JAR path
    #[arg(long)]
    pub kotlin_jar: Option<String>,

    /// Run specific parity test by number (1-12)
    #[arg(long)]
    pub test_number: Option<usize>,

    /// Tolerance for floating point comparisons
    #[arg(long, default_value = "0.001")]
    pub tolerance: f64,

    /// Use mock Bitcoin RPC instead of real
    #[arg(long)]
    pub use_mock_rpc: bool,

    /// Port for mock RPC server
    #[arg(long, default_value = "18332")]
    pub mock_rpc_port: u16,

    /// Timeout for server startup in seconds
    #[arg(long, default_value = "30")]
    pub startup_timeout: u64,
}
