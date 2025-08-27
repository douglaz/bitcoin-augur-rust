#![allow(dead_code)]

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, trace};

/// API client for bitcoin-augur-server
pub struct ApiClient {
    client: Client,
    base_url: String,
}

impl ApiClient {
    /// Create a new API client
    pub fn new(base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");

        Self { client, base_url }
    }

    /// Get fee estimates for all targets
    pub async fn get_fees(&self) -> Result<FeeEstimateResponse> {
        let url = format!("{base_url}/fees", base_url = self.base_url);
        debug!("Getting fee estimates from {url}");

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to send request")?;

        if response.status() == StatusCode::SERVICE_UNAVAILABLE {
            return Ok(FeeEstimateResponse::empty());
        }

        response
            .json()
            .await
            .context("Failed to parse fee estimates")
    }

    /// Get fee estimates for specific block target
    pub async fn get_fees_for_target(&self, num_blocks: f64) -> Result<FeeEstimateResponse> {
        let url = format!(
            "{base_url}/fees/target/{num_blocks}",
            base_url = self.base_url
        );
        debug!("Getting fee estimates for {num_blocks} blocks from {url}");

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to send request")?;

        if response.status() == StatusCode::SERVICE_UNAVAILABLE {
            return Ok(FeeEstimateResponse::empty());
        }

        response
            .json()
            .await
            .context("Failed to parse fee estimates")
    }

    /// Get raw response as JSON value (for compatibility testing)
    pub async fn get_raw(&self, path: &str) -> Result<(StatusCode, Value)> {
        let url = format!("{base_url}{path}", base_url = self.base_url);
        trace!("Getting raw response from {url}");

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to send request")?;

        let status = response.status();

        // Try to parse as JSON, fall back to string if not JSON
        let body = if status.is_success() || status == StatusCode::SERVICE_UNAVAILABLE {
            match response.json::<Value>().await {
                Ok(json) => json,
                Err(_) => {
                    // If not JSON, return as string value
                    Value::String("Non-JSON response".to_string())
                }
            }
        } else {
            let text = response.text().await.unwrap_or_default();
            Value::String(text)
        };

        Ok((status, body))
    }

    /// Check server health
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{base_url}/health", base_url = self.base_url);
        let response = self.client.get(&url).send().await?;
        Ok(response.status().is_success())
    }
}

/// Fee estimate response structure (matches both Rust and Kotlin implementations)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FeeEstimateResponse {
    #[serde(rename = "mempool_update_time")]
    pub mempool_update_time: String,
    pub estimates: HashMap<String, BlockTarget>,
}

impl FeeEstimateResponse {
    /// Create an empty response
    pub fn empty() -> Self {
        Self {
            mempool_update_time: Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            estimates: HashMap::new(),
        }
    }

    /// Check if response has estimates
    pub fn has_estimates(&self) -> bool {
        !self.estimates.is_empty()
    }

    /// Get estimate for specific block target
    pub fn get_target(&self, blocks: usize) -> Option<&BlockTarget> {
        self.estimates.get(&blocks.to_string())
    }
}

/// Block target with probability-based fee estimates
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlockTarget {
    pub probabilities: HashMap<String, Probability>,
}

impl BlockTarget {
    /// Get fee rate for specific probability
    pub fn get_fee_rate(&self, probability: f64) -> Option<f64> {
        let key = format!("{:.2}", probability);
        self.probabilities.get(&key).map(|p| p.fee_rate)
    }
}

/// Probability with fee rate
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Probability {
    #[serde(rename = "fee_rate")]
    pub fee_rate: f64,
}

/// Helper for comparing API responses
pub struct ResponseComparator;

impl ResponseComparator {
    /// Compare two fee estimate responses for compatibility
    pub fn compare_fee_estimates(
        resp1: &FeeEstimateResponse,
        resp2: &FeeEstimateResponse,
    ) -> Result<Vec<String>> {
        let mut differences = Vec::new();

        // Compare estimates availability
        if resp1.has_estimates() != resp2.has_estimates() {
            differences.push(format!(
                "Estimates availability mismatch: {has1} vs {has2}",
                has1 = resp1.has_estimates(),
                has2 = resp2.has_estimates()
            ));
            return Ok(differences);
        }

        // Compare block targets
        let keys1: Vec<_> = resp1.estimates.keys().cloned().collect();
        let keys2: Vec<_> = resp2.estimates.keys().cloned().collect();

        if keys1 != keys2 {
            differences.push(format!("Block targets mismatch: {keys1:?} vs {keys2:?}"));
        }

        // Compare each target
        for (block_num, target1) in &resp1.estimates {
            if let Some(target2) = resp2.estimates.get(block_num) {
                Self::compare_block_targets(block_num, target1, target2, &mut differences);
            }
        }

        Ok(differences)
    }

    /// Compare block targets
    fn compare_block_targets(
        block_num: &str,
        target1: &BlockTarget,
        target2: &BlockTarget,
        differences: &mut Vec<String>,
    ) {
        let probs1: Vec<_> = target1.probabilities.keys().cloned().collect();
        let probs2: Vec<_> = target2.probabilities.keys().cloned().collect();

        if probs1 != probs2 {
            differences.push(format!(
                "Block {block_num} probabilities mismatch: {probs1:?} vs {probs2:?}"
            ));
        }

        // Compare fee rates with tolerance
        for (prob_str, prob1) in &target1.probabilities {
            if let Some(prob2) = target2.probabilities.get(prob_str) {
                let diff = (prob1.fee_rate - prob2.fee_rate).abs();
                let tolerance = 0.0001; // Allow small floating point differences

                if diff > tolerance {
                    differences.push(format!(
                        "Block {block_num} probability {prob} fee rate mismatch: {fee1:.4} vs {fee2:.4}",
                        prob = prob_str,
                        fee1 = prob1.fee_rate,
                        fee2 = prob2.fee_rate
                    ));
                }
            }
        }
    }

    /// Compare raw JSON responses
    pub fn compare_json(val1: &Value, val2: &Value, path: &str) -> Vec<String> {
        let mut differences = Vec::new();
        Self::compare_json_recursive(val1, val2, path, &mut differences);
        differences
    }

    fn compare_json_recursive(
        val1: &Value,
        val2: &Value,
        path: &str,
        differences: &mut Vec<String>,
    ) {
        match (val1, val2) {
            (Value::Object(map1), Value::Object(map2)) => {
                let all_keys: std::collections::HashSet<_> =
                    map1.keys().chain(map2.keys()).collect();

                for key in all_keys {
                    let new_path = format!("{path}.{key}");
                    match (map1.get(key), map2.get(key)) {
                        (Some(v1), Some(v2)) => {
                            Self::compare_json_recursive(v1, v2, &new_path, differences);
                        }
                        (Some(_), None) => {
                            differences
                                .push(format!("{new_path}: present in first, missing in second"));
                        }
                        (None, Some(_)) => {
                            differences
                                .push(format!("{new_path}: missing in first, present in second"));
                        }
                        _ => {}
                    }
                }
            }
            (Value::Array(arr1), Value::Array(arr2)) => {
                if arr1.len() != arr2.len() {
                    differences.push(format!(
                        "{path}: array length mismatch ({len1} vs {len2})",
                        len1 = arr1.len(),
                        len2 = arr2.len()
                    ));
                } else {
                    for (i, (item1, item2)) in arr1.iter().zip(arr2.iter()).enumerate() {
                        let new_path = format!("{path}[{i}]");
                        Self::compare_json_recursive(item1, item2, &new_path, differences);
                    }
                }
            }
            (Value::Number(n1), Value::Number(n2)) => {
                if let (Some(f1), Some(f2)) = (n1.as_f64(), n2.as_f64()) {
                    let diff = (f1 - f2).abs();
                    let tolerance = 0.0001;
                    if diff > tolerance {
                        differences.push(format!("{path}: number mismatch ({f1} vs {f2})"));
                    }
                } else if n1 != n2 {
                    differences.push(format!("{path}: number mismatch ({n1} vs {n2})"));
                }
            }
            (Value::String(s1), Value::String(s2)) => {
                // Special handling for timestamps
                if path.contains("time") {
                    // Just check both are valid timestamps, don't compare exact values
                    let valid1 = DateTime::parse_from_rfc3339(s1).is_ok();
                    let valid2 = DateTime::parse_from_rfc3339(s2).is_ok();
                    if !valid1 || !valid2 {
                        differences.push(format!("{path}: invalid timestamp format"));
                    }
                } else if s1 != s2 {
                    differences.push(format!("{path}: string mismatch ({s1} vs {s2})"));
                }
            }
            (v1, v2) if v1 != v2 => {
                differences.push(format!("{path}: value mismatch"));
            }
            _ => {}
        }
    }
}
