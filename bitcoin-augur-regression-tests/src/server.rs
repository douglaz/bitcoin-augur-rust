use anyhow::{bail, ensure, Context, Result};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::time::{sleep, timeout};
use tracing::{debug, info};

/// Manages a bitcoin-augur-server process for testing
pub struct ServerManager {
    process: Option<Child>,
    port: u16,
    binary_path: PathBuf,
    data_dir: PathBuf,
}

impl ServerManager {
    /// Create a new server manager
    pub fn new(binary_path: PathBuf, port: u16, data_dir: PathBuf) -> Self {
        Self {
            process: None,
            port,
            binary_path,
            data_dir,
        }
    }

    /// Start the server process
    pub async fn start(&mut self) -> Result<()> {
        ensure!(self.process.is_none(), "Server is already running");

        info!(
            "Starting bitcoin-augur-server on port {port}",
            port = self.port
        );

        // Create config for test server
        let config_path = self.data_dir.join("test-config.yaml");
        self.write_test_config(&config_path).await?;

        // Start the server process
        let mut cmd = Command::new(&self.binary_path);
        cmd.env("AUGUR_SERVER_CONFIG", &config_path)
            .env("AUGUR_SERVER_PORT", self.port.to_string())
            .env(
                "AUGUR_PERSISTENCE_DATA_DIRECTORY",
                self.data_dir.join("mempool"),
            )
            .env("AUGUR_TEST_MODE_ENABLED", "true") // Enable test mode
            .env("AUGUR_TEST_MODE_USE_MOCK_DATA", "true") // Use mock data
            .env("RUST_LOG", "info")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let child = cmd.spawn().with_context(|| {
            format!(
                "Failed to start server from {path:?}",
                path = self.binary_path
            )
        })?;

        self.process = Some(child);

        // Wait for server to be ready
        self.wait_for_ready().await?;

        Ok(())
    }

    /// Stop the server process
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(mut process) = self.process.take() {
            info!("Stopping bitcoin-augur-server");

            // Try graceful shutdown first
            process.kill().await.ok();

            // Wait a bit for process to terminate
            sleep(Duration::from_millis(500)).await;
        }
        Ok(())
    }

    /// Check if server is running
    #[allow(dead_code)]
    pub async fn is_running(&self) -> bool {
        // Try to connect to the health endpoint
        let url = format!("http://127.0.0.1:{port}/health", port = self.port);
        reqwest::get(&url).await.is_ok()
    }

    /// Wait for server to be ready
    async fn wait_for_ready(&self) -> Result<()> {
        let url = format!("http://127.0.0.1:{port}/health", port = self.port);
        let max_wait = Duration::from_secs(30);
        let check_interval = Duration::from_millis(500);

        info!("Waiting for server to be ready at {url}");

        let start = std::time::Instant::now();
        loop {
            if start.elapsed() > max_wait {
                bail!("Server failed to start within {max_wait:?}");
            }

            match timeout(Duration::from_secs(1), reqwest::get(&url)).await {
                Ok(Ok(response)) if response.status().is_success() => {
                    info!("Server is ready");
                    return Ok(());
                }
                _ => {
                    debug!("Server not ready yet, retrying...");
                    sleep(check_interval).await;
                }
            }
        }
    }

    /// Write test configuration
    async fn write_test_config(&self, path: &PathBuf) -> Result<()> {
        let config = format!(
            r#"# Test configuration for bitcoin-augur-server
server:
  port: {}
  host: "127.0.0.1"

mempool:
  refresh_interval_secs: 5
  data_path: "{}"
  max_snapshots: 100

bitcoin:
  rpc_url: "http://localhost:38332"
  rpc_user: "test"
  rpc_password: "test"

logging:
  level: "info"
  format: "json"
"#,
            self.port,
            self.data_dir.join("mempool").display()
        );

        tokio::fs::create_dir_all(path.parent().unwrap()).await?;
        tokio::fs::write(path, config).await?;
        Ok(())
    }

    /// Get the server URL
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{port}", port = self.port)
    }
}

impl Drop for ServerManager {
    fn drop(&mut self) {
        // Ensure server is stopped when manager is dropped
        if let Some(mut process) = self.process.take() {
            let _ = process.start_kill();
        }
    }
}

/// Manager for reference implementation (Java)
pub struct ReferenceServerManager {
    process: Option<Child>,
    port: u16,
    jar_path: PathBuf,
    data_dir: PathBuf,
}

impl ReferenceServerManager {
    /// Create a new reference server manager
    pub fn new(jar_path: PathBuf, port: u16, data_dir: PathBuf) -> Self {
        Self {
            process: None,
            port,
            jar_path,
            data_dir,
        }
    }

    /// Start the reference server
    pub async fn start(&mut self) -> Result<()> {
        ensure!(
            self.process.is_none(),
            "Reference server is already running"
        );

        info!("Starting reference server on port {port}", port = self.port);

        // Create config for reference server
        let config_path = self.data_dir.join("reference-config.yaml");
        self.write_test_config(&config_path).await?;

        // Start the Java process
        let mut cmd = Command::new("java");
        cmd.arg("-jar")
            .arg(&self.jar_path)
            .env("APP_CONFIG", &config_path)
            .env("SERVER_PORT", self.port.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let child = cmd.spawn().with_context(|| {
            format!(
                "Failed to start reference server from {path:?}",
                path = self.jar_path
            )
        })?;

        self.process = Some(child);

        // Wait for server to be ready
        self.wait_for_ready().await?;

        Ok(())
    }

    /// Stop the reference server
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(mut process) = self.process.take() {
            info!("Stopping reference server");
            process.kill().await.ok();
            sleep(Duration::from_millis(500)).await;
        }
        Ok(())
    }

    /// Wait for server to be ready
    async fn wait_for_ready(&self) -> Result<()> {
        let url = format!("http://127.0.0.1:{port}/fees", port = self.port);
        let max_wait = Duration::from_secs(60); // Java server may take longer
        let check_interval = Duration::from_secs(1);

        info!("Waiting for reference server to be ready at {url}");

        let start = std::time::Instant::now();
        loop {
            if start.elapsed() > max_wait {
                bail!("Reference server failed to start within {max_wait:?}");
            }

            match timeout(Duration::from_secs(2), reqwest::get(&url)).await {
                Ok(Ok(response)) => {
                    // Reference server returns 503 when no data available (which is OK for startup)
                    if response.status().is_success() || response.status() == 503 {
                        info!("Reference server is ready");
                        return Ok(());
                    }
                }
                _ => {
                    debug!("Reference server not ready yet, retrying...");
                }
            }

            sleep(check_interval).await;
        }
    }

    /// Write test configuration for reference server
    async fn write_test_config(&self, path: &PathBuf) -> Result<()> {
        let config = format!(
            r#"# Test configuration for reference server
server:
  port: {}
  host: "127.0.0.1"

mempool:
  refreshIntervalSecs: 5
  dataPath: "{}"
  maxSnapshots: 100

bitcoin:
  rpcUrl: "http://localhost:38332"
  rpcUser: "test"
  rpcPassword: "test"

logging:
  level: "INFO"
"#,
            self.port,
            self.data_dir.join("mempool-ref").display()
        );

        tokio::fs::create_dir_all(path.parent().unwrap()).await?;
        tokio::fs::write(path, config).await?;
        Ok(())
    }

    /// Get the server URL
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{port}", port = self.port)
    }
}

impl Drop for ReferenceServerManager {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            let _ = process.start_kill();
        }
    }
}
