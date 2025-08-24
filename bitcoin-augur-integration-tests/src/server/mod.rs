mod kotlin_server;
mod rust_server;

pub use kotlin_server::KotlinServer;
pub use rust_server::RustServer;

use anyhow::Result;
use async_trait::async_trait;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

#[async_trait]
pub trait Server: Send + Sync {
    /// Start the server process
    async fn start(&mut self) -> Result<()>;

    /// Stop the server process
    async fn stop(&mut self) -> Result<()>;

    /// Check if the server is healthy
    async fn health_check(&self) -> Result<bool>;

    /// Get the server's base URL
    fn base_url(&self) -> String;

    /// Get server name for logging
    fn name(&self) -> &str;

    /// Wait for the server to be ready
    async fn wait_for_ready(&self, timeout: Duration) -> Result<()> {
        let start = std::time::Instant::now();

        info!("Waiting for {} to be ready...", self.name());

        loop {
            if start.elapsed() > timeout {
                anyhow::bail!("{} failed to start within {:?}", self.name(), timeout);
            }

            match self.health_check().await {
                Ok(true) => {
                    info!("{} is ready", self.name());
                    return Ok(());
                }
                Ok(false) => {
                    warn!("{} health check returned false", self.name());
                }
                Err(e) => {
                    // Expected during startup
                    tracing::debug!("{} health check failed: {}", self.name(), e);
                }
            }

            sleep(Duration::from_secs(1)).await;
        }
    }
}
