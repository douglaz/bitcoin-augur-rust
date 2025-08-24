use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use std::process::Stdio;
use tempfile::TempDir;
use tokio::process::{Child, Command};
use tracing::{debug, info};

use super::Server;

pub struct RustServer {
    port: u16,
    binary_path: PathBuf,
    process: Option<Child>,
    temp_dir: Option<TempDir>,
    bitcoin_rpc: String,
    rpc_user: Option<String>,
    rpc_password: Option<String>,
    data_dir: Option<PathBuf>,
}

impl RustServer {
    pub fn new(
        port: u16,
        binary_path: Option<String>,
        bitcoin_rpc: String,
        rpc_user: Option<String>,
        rpc_password: Option<String>,
    ) -> Result<Self> {
        let binary_path = if let Some(path) = binary_path {
            PathBuf::from(path)
        } else {
            find_rust_binary()?
        };

        Ok(Self {
            port,
            binary_path,
            process: None,
            temp_dir: None,
            bitcoin_rpc,
            rpc_user,
            rpc_password,
            data_dir: None,
        })
    }

    /// Set a pre-populated data directory to use instead of creating a temporary one
    pub fn set_data_directory(&mut self, data_dir: PathBuf) {
        self.data_dir = Some(data_dir);
    }
}

#[async_trait]
impl Server for RustServer {
    async fn start(&mut self) -> Result<()> {
        if self.process.is_some() {
            return Ok(());
        }

        info!("Starting Rust server on port {}", self.port);

        // Use provided data directory or create temporary one
        let (temp_dir, data_dir) = if let Some(ref data_dir) = self.data_dir {
            // Use pre-populated data directory
            info!("Using pre-populated data directory: {}", data_dir.display());
            (None, data_dir.clone())
        } else {
            // Create temporary directory for data
            let temp_dir = TempDir::new()?;
            let data_dir = temp_dir.path().join("mempool_data");
            std::fs::create_dir_all(&data_dir)?;
            (Some(temp_dir), data_dir)
        };

        // Create config file
        let config_dir = if let Some(ref td) = temp_dir {
            td.path().to_path_buf()
        } else {
            // Use data directory's parent for config when using pre-populated data
            data_dir.parent().unwrap_or(&data_dir).to_path_buf()
        };
        let config_path = config_dir.join("config.yaml");
        let config_content = format!(
            r#"
server:
  host: "127.0.0.1"
  port: {}

bitcoin_rpc:
  url: "{}"
  username: "{}"
  password: "{}"

persistence:
  data_directory: "{}"
  cleanup_days: 7

collector:
  interval_ms: 1000  # Fast polling for tests
"#,
            self.port,
            self.bitcoin_rpc,
            self.rpc_user.as_deref().unwrap_or(""),
            self.rpc_password.as_deref().unwrap_or(""),
            data_dir.display()
        );

        std::fs::write(&config_path, config_content)?;

        // Start the process
        let mut cmd = Command::new(&self.binary_path);

        // Add --init-from-store flag if we have pre-populated data
        if self.data_dir.is_some() {
            cmd.arg("--init-from-store");
        }

        cmd.env("AUGUR_CONFIG_FILE", config_path.to_str().unwrap())
            .env("RUST_LOG", "info")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let child = cmd.spawn().with_context(|| {
            format!(
                "Failed to start Rust server at {}",
                self.binary_path.display()
            )
        })?;

        self.process = Some(child);
        if temp_dir.is_some() {
            self.temp_dir = temp_dir;
        }

        debug!("Rust server process started");
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        if let Some(mut process) = self.process.take() {
            info!("Stopping Rust server");
            process.kill().await?;
            process.wait().await?;
        }

        self.temp_dir = None;
        Ok(())
    }

    async fn health_check(&self) -> Result<bool> {
        let client = reqwest::Client::new();
        let url = format!("{}/health", self.base_url());

        let response = client
            .get(&url)
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn name(&self) -> &str {
        "Rust server"
    }
}

fn find_rust_binary() -> Result<PathBuf> {
    // First try to find in PATH
    if let Ok(path) = which::which("bitcoin-augur-server") {
        return Ok(path);
    }

    // Try common build locations - prioritize musl builds
    let possible_paths = vec![
        // Prioritize musl builds (static linking)
        "target/x86_64-unknown-linux-musl/release/bitcoin-augur-server",
        "target/x86_64-unknown-linux-musl/debug/bitcoin-augur-server",
        "../target/x86_64-unknown-linux-musl/release/bitcoin-augur-server",
        "../target/x86_64-unknown-linux-musl/debug/bitcoin-augur-server",
        // Fallback to generic target builds
        "target/release/bitcoin-augur-server",
        "target/debug/bitcoin-augur-server",
        "../target/release/bitcoin-augur-server",
        "../target/debug/bitcoin-augur-server",
    ];

    for path_str in possible_paths {
        let path = PathBuf::from(path_str);
        if path.exists() && path.is_file() {
            return Ok(path.canonicalize()?);
        }
    }

    anyhow::bail!("Could not find bitcoin-augur-server binary. Please build it with: cargo build --release -p bitcoin-augur-server")
}
