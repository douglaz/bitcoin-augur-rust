use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use std::process::Stdio;
use tempfile::TempDir;
use tokio::process::{Child, Command};
use tracing::{debug, info};

use super::Server;

pub struct KotlinServer {
    port: u16,
    jar_path: PathBuf,
    process: Option<Child>,
    temp_dir: Option<TempDir>,
    bitcoin_rpc: String,
    rpc_user: Option<String>,
    rpc_password: Option<String>,
}

impl KotlinServer {
    pub fn new(
        port: u16,
        jar_path: Option<String>,
        bitcoin_rpc: String,
        rpc_user: Option<String>,
        rpc_password: Option<String>,
    ) -> Result<Self> {
        let jar_path = if let Some(path) = jar_path {
            PathBuf::from(path)
        } else {
            find_kotlin_jar()?
        };

        Ok(Self {
            port,
            jar_path,
            process: None,
            temp_dir: None,
            bitcoin_rpc,
            rpc_user,
            rpc_password,
        })
    }
}

#[async_trait]
impl Server for KotlinServer {
    async fn start(&mut self) -> Result<()> {
        if self.process.is_some() {
            return Ok(());
        }

        info!("Starting Kotlin server on port {}", self.port);

        // Create temporary directory for data
        let temp_dir = TempDir::new()?;
        let data_dir = temp_dir.path().join("mempool_data");
        std::fs::create_dir_all(&data_dir)?;

        // Create config file
        let config_path = temp_dir.path().join("config.yaml");
        let config_content = format!(
            r#"server:
  host: "127.0.0.1"
  port: {}
bitcoinRpc:
  url: "{}"
  username: "{}"
  password: "{}"
persistence:
  dataDirectory: "{}"
"#,
            self.port,
            self.bitcoin_rpc,
            self.rpc_user.as_deref().unwrap_or(""),
            self.rpc_password.as_deref().unwrap_or(""),
            data_dir.display()
        );

        std::fs::write(&config_path, config_content)?;

        // Start the process
        let mut cmd = Command::new("java");
        cmd.arg("-jar")
            .arg(&self.jar_path)
            .env("AUGUR_CONFIG_FILE", config_path.to_str().unwrap())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let child = cmd.spawn().with_context(|| {
            format!(
                "Failed to start Kotlin server with JAR {}",
                self.jar_path.display()
            )
        })?;

        self.process = Some(child);
        self.temp_dir = Some(temp_dir);

        debug!("Kotlin server process started");
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        if let Some(mut process) = self.process.take() {
            info!("Stopping Kotlin server");
            process.kill().await?;
            process.wait().await?;
        }

        self.temp_dir = None;
        Ok(())
    }

    async fn health_check(&self) -> Result<bool> {
        let client = reqwest::Client::new();
        let url = format!("{}/fees", self.base_url());

        // Kotlin server might not have a /health endpoint, try /fees
        let response = client
            .get(&url)
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await?;

        // Accept either 200 or 503 (no data) as "healthy"
        Ok(response.status().is_success() || response.status() == 503)
    }

    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn name(&self) -> &str {
        "Kotlin server"
    }
}

fn find_kotlin_jar() -> Result<PathBuf> {
    // Try common build locations - prioritize shadow JAR
    let possible_paths = vec![
        // Shadow JAR (includes all dependencies)
        "bitcoin-augur-reference/app/build/libs/app-all.jar",
        "../bitcoin-augur-reference/app/build/libs/app-all.jar",
        "../../bitcoin-augur-reference/app/build/libs/app-all.jar",
        // Regular JAR (might not work without dependencies)
        "bitcoin-augur-reference/app/build/libs/app.jar",
        "../bitcoin-augur-reference/app/build/libs/app.jar",
        "../../bitcoin-augur-reference/app/build/libs/app.jar",
        "bitcoin-augur-reference/build/libs/app.jar",
        "../bitcoin-augur-reference/build/libs/app.jar",
        "../../bitcoin-augur-reference/build/libs/app.jar",
    ];

    for path_str in possible_paths {
        let path = PathBuf::from(path_str);
        if path.exists() && path.is_file() {
            return Ok(path.canonicalize()?);
        }
    }

    anyhow::bail!("Could not find bitcoin-augur-reference JAR. Please build it with: ./bitcoin-augur-integration-tests build-kotlin")
}
