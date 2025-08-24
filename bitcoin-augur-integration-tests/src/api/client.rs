use anyhow::{Context, Result};
use reqwest::Client;
use std::time::Duration;
use tracing::debug;

use super::models::FeeEstimateResponse;

pub struct ApiClient {
    client: Client,
    base_url: String,
}

impl ApiClient {
    pub fn new(base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();

        Self { client, base_url }
    }

    /// Get current fee estimates
    pub async fn get_fees(&self) -> Result<FeeEstimateResponse> {
        let url = format!("{}/fees", self.base_url);
        debug!("Fetching fees from {}", url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to send request")?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("Request failed with status {}: {}", status, text);
        }

        let fees = response
            .json::<FeeEstimateResponse>()
            .await
            .context("Failed to parse response")?;

        Ok(fees)
    }

    /// Get fee estimate for specific block target
    pub async fn get_fee_for_target(&self, blocks: u32) -> Result<FeeEstimateResponse> {
        let url = format!("{}/fees/target/{}", self.base_url, blocks);
        debug!("Fetching fee for {} blocks from {}", blocks, url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to send request")?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("Request failed with status {}: {}", status, text);
        }

        let fees = response
            .json::<FeeEstimateResponse>()
            .await
            .context("Failed to parse response")?;

        Ok(fees)
    }

    /// Get historical fee estimate
    #[allow(dead_code)]
    pub async fn get_historical_fee(&self, timestamp: i64) -> Result<FeeEstimateResponse> {
        let url = format!("{}/historical_fee?timestamp={}", self.base_url, timestamp);
        debug!(
            "Fetching historical fee for timestamp {} from {}",
            timestamp, url
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to send request")?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("Request failed with status {}: {}", status, text);
        }

        let fees = response
            .json::<FeeEstimateResponse>()
            .await
            .context("Failed to parse response")?;

        Ok(fees)
    }

    /// Check if server is healthy
    pub async fn health_check(&self) -> Result<bool> {
        // Try /health endpoint first
        let url = format!("{}/health", self.base_url);
        let response = self.client.get(&url).send().await;

        if let Ok(resp) = response {
            if resp.status().is_success() {
                return Ok(true);
            }
        }

        // Fallback to /fees endpoint
        let url = format!("{}/fees", self.base_url);
        let response = self.client.get(&url).send().await?;

        // Accept 503 (no data) as healthy
        Ok(response.status().is_success() || response.status() == 503)
    }
}
