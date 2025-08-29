//! Mock Bitcoin RPC server for deterministic testing
//!
//! Provides a controllable Bitcoin Core RPC mock that can simulate
//! various mempool states for testing fee estimation algorithms.

#![allow(dead_code)]

use anyhow::Result;
use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{Arc, RwLock};
use tracing::{debug, info};

/// A mock transaction in the mempool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockTransaction {
    pub txid: String,
    pub weight: u32,
    pub fee: u64,
    pub fee_rate: f64,
}

impl MockTransaction {
    pub fn new(weight: u32, fee: u64) -> Self {
        let fee_rate = (fee as f64 / weight as f64) * 4.0; // Convert to sat/vB
        Self {
            txid: format!("{:064x}", rand::random::<u64>()),
            weight,
            fee,
            fee_rate,
        }
    }
}

/// Mock Bitcoin RPC server state
pub struct MockBitcoinRpc {
    mempool: Arc<RwLock<Vec<MockTransaction>>>,
    block_height: Arc<RwLock<u64>>,
    port: u16,
}

impl MockBitcoinRpc {
    /// Create a new mock RPC server
    pub fn new(port: u16) -> Self {
        Self {
            mempool: Arc::new(RwLock::new(Vec::new())),
            block_height: Arc::new(RwLock::new(850000)),
            port,
        }
    }

    /// Set the mempool state
    pub fn set_mempool(&self, transactions: Vec<MockTransaction>) {
        *self.mempool.write().unwrap() = transactions;
    }

    /// Add a transaction to the mempool
    pub fn add_transaction(&self, tx: MockTransaction) {
        self.mempool.write().unwrap().push(tx);
    }

    /// Clear the mempool
    pub fn clear_mempool(&self) {
        self.mempool.write().unwrap().clear();
    }

    /// Set the block height
    pub fn set_block_height(&self, height: u64) {
        *self.block_height.write().unwrap() = height;
    }

    /// Start the mock RPC server
    pub async fn start(self: Arc<Self>) -> Result<()> {
        let state = MockRpcState {
            mempool: self.mempool.clone(),
            block_height: self.block_height.clone(),
        };

        let app = Router::new().route("/", post(handle_rpc)).with_state(state);

        let addr = format!("127.0.0.1:{}", self.port);
        info!("Mock Bitcoin RPC server listening on {addr}");

        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }

    /// Get the RPC URL for this mock server
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

#[derive(Clone)]
struct MockRpcState {
    mempool: Arc<RwLock<Vec<MockTransaction>>>,
    block_height: Arc<RwLock<u64>>,
}

#[derive(Deserialize)]
struct RpcRequest {
    method: String,
    params: Option<Value>,
    id: Value,
}

#[derive(Serialize)]
struct RpcResponse {
    result: Option<Value>,
    error: Option<Value>,
    id: Value,
}

async fn handle_rpc(
    State(state): State<MockRpcState>,
    Json(request): Json<RpcRequest>,
) -> Result<Json<RpcResponse>, StatusCode> {
    debug!("Mock RPC received method: {}", request.method);

    let result = match request.method.as_str() {
        "getblockchaininfo" => {
            let height = *state.block_height.read().unwrap();
            Some(json!({
                "chain": "main",
                "blocks": height,
                "headers": height,
                "bestblockhash": format!("{:064x}", height),
                "difficulty": 1.0,
                "time": chrono::Utc::now().timestamp(),
                "mediantime": chrono::Utc::now().timestamp() - 600,
                "verificationprogress": 0.999999,
                "initialblockdownload": false,
                "chainwork": format!("{:064x}", height * 1000),
                "size_on_disk": 500000000000u64,
                "pruned": false
            }))
        }
        "getrawmempool" => {
            let mempool = state.mempool.read().unwrap();
            let txids: Vec<String> = mempool.iter().map(|tx| tx.txid.clone()).collect();

            // Check if verbose parameter is true
            if let Some(params) = request.params {
                if let Some(verbose) = params
                    .as_array()
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_bool())
                {
                    if verbose {
                        // Return verbose mempool info
                        let mut verbose_mempool = serde_json::Map::new();
                        for tx in mempool.iter() {
                            verbose_mempool.insert(
                                tx.txid.clone(),
                                json!({
                                    "vsize": tx.weight / 4,
                                    "weight": tx.weight,
                                    "fee": tx.fee as f64 / 100_000_000.0, // Convert to BTC
                                    "modifiedfee": tx.fee as f64 / 100_000_000.0,
                                    "time": chrono::Utc::now().timestamp() - 300,
                                    "height": *state.block_height.read().unwrap(),
                                    "descendantcount": 1,
                                    "descendantsize": tx.weight / 4,
                                    "descendantfees": tx.fee,
                                    "ancestorcount": 1,
                                    "ancestorsize": tx.weight / 4,
                                    "ancestorfees": tx.fee,
                                    "fees": {
                                        "base": tx.fee as f64 / 100_000_000.0,
                                        "modified": tx.fee as f64 / 100_000_000.0,
                                        "ancestor": tx.fee as f64 / 100_000_000.0,
                                        "descendant": tx.fee as f64 / 100_000_000.0
                                    }
                                }),
                            );
                        }
                        return Ok(Json(RpcResponse {
                            result: Some(Value::Object(verbose_mempool)),
                            error: None,
                            id: request.id,
                        }));
                    }
                }
            }

            Some(json!(txids))
        }
        "getmempoolentry" => {
            if let Some(params) = request.params {
                if let Some(txid) = params
                    .as_array()
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_str())
                {
                    let mempool = state.mempool.read().unwrap();
                    if let Some(tx) = mempool.iter().find(|t| t.txid == txid) {
                        return Ok(Json(RpcResponse {
                            result: Some(json!({
                                "vsize": tx.weight / 4,
                                "weight": tx.weight,
                                "fee": tx.fee as f64 / 100_000_000.0,
                                "modifiedfee": tx.fee as f64 / 100_000_000.0,
                                "time": chrono::Utc::now().timestamp() - 300,
                                "height": *state.block_height.read().unwrap(),
                                "descendantcount": 1,
                                "descendantsize": tx.weight / 4,
                                "descendantfees": tx.fee,
                                "ancestorcount": 1,
                                "ancestorsize": tx.weight / 4,
                                "ancestorfees": tx.fee,
                            })),
                            error: None,
                            id: request.id,
                        }));
                    }
                }
            }
            None
        }
        _ => {
            return Ok(Json(RpcResponse {
                result: None,
                error: Some(json!({
                    "code": -32601,
                    "message": "Method not found"
                })),
                id: request.id,
            }));
        }
    };

    Ok(Json(RpcResponse {
        result,
        error: None,
        id: request.id,
    }))
}

/// Test data generator for creating various mempool scenarios
pub struct TestDataGenerator;

impl TestDataGenerator {
    /// Generate an empty mempool
    pub fn empty_mempool() -> Vec<MockTransaction> {
        vec![]
    }

    /// Generate a single transaction
    pub fn single_transaction() -> Vec<MockTransaction> {
        vec![MockTransaction::new(1000, 10000)]
    }

    /// Generate a uniform distribution of fees
    pub fn uniform_distribution(
        count: usize,
        min_fee_rate: f64,
        max_fee_rate: f64,
    ) -> Vec<MockTransaction> {
        let mut txs = Vec::new();
        let step = (max_fee_rate - min_fee_rate) / count as f64;

        for i in 0..count {
            let fee_rate = min_fee_rate + (i as f64 * step);
            let weight = 1000 + (i as u32 * 100);
            let fee = (fee_rate * weight as f64 / 4.0) as u64;
            txs.push(MockTransaction::new(weight, fee));
        }

        txs
    }

    /// Generate a bimodal distribution (two peaks)
    pub fn bimodal_distribution(count: usize) -> Vec<MockTransaction> {
        let mut txs = Vec::new();
        let half = count / 2;

        // First peak around 5 sat/vB
        for i in 0..half {
            let fee_rate = 4.0 + (i as f64 * 0.5 / half as f64);
            let weight = 1000 + (i as u32 * 50);
            let fee = (fee_rate * weight as f64 / 4.0) as u64;
            txs.push(MockTransaction::new(weight, fee));
        }

        // Second peak around 20 sat/vB
        for i in 0..half {
            let fee_rate = 18.0 + (i as f64 * 4.0 / half as f64);
            let weight = 1500 + (i as u32 * 50);
            let fee = (fee_rate * weight as f64 / 4.0) as u64;
            txs.push(MockTransaction::new(weight, fee));
        }

        txs
    }

    /// Generate a fee spike scenario
    pub fn fee_spike(base_count: usize, spike_count: usize) -> Vec<MockTransaction> {
        let mut txs = Vec::new();

        // Base load at low fees (1-5 sat/vB)
        for i in 0..base_count {
            let fee_rate = 1.0 + (i as f64 * 4.0 / base_count as f64);
            let weight = 1000 + (i as u32 * 100);
            let fee = (fee_rate * weight as f64 / 4.0) as u64;
            txs.push(MockTransaction::new(weight, fee));
        }

        // Spike at high fees (50-100 sat/vB)
        for i in 0..spike_count {
            let fee_rate = 50.0 + (i as f64 * 50.0 / spike_count as f64);
            let weight = 800 + (i as u32 * 50);
            let fee = (fee_rate * weight as f64 / 4.0) as u64;
            txs.push(MockTransaction::new(weight, fee));
        }

        txs
    }

    /// Generate graduated fees (steadily increasing)
    pub fn graduated_fees(count: usize) -> Vec<MockTransaction> {
        let mut txs = Vec::new();

        for i in 0..count {
            // Exponential growth from 1 to 100 sat/vB
            let progress = i as f64 / count as f64;
            let fee_rate = (100_f64.powf(progress) - 1.0) / 99.0 * 99.0 + 1.0;
            let weight = 1000 + (i as u32 * 50);
            let fee = (fee_rate * weight as f64 / 4.0) as u64;
            txs.push(MockTransaction::new(weight, fee));
        }

        txs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_transaction_creation() -> Result<()> {
        let tx = MockTransaction::new(1000, 10000);
        assert_eq!(tx.weight, 1000);
        assert_eq!(tx.fee, 10000);
        assert!((tx.fee_rate - 40.0).abs() < 0.001); // 10000 / (1000/4) = 40 sat/vB
        Ok(())
    }

    #[test]
    fn test_data_generators() -> Result<()> {
        // Test empty mempool
        let empty = TestDataGenerator::empty_mempool();
        assert_eq!(empty.len(), 0);

        // Test single transaction
        let single = TestDataGenerator::single_transaction();
        assert_eq!(single.len(), 1);

        // Test uniform distribution
        let uniform = TestDataGenerator::uniform_distribution(10, 1.0, 10.0);
        assert_eq!(uniform.len(), 10);

        // Test bimodal distribution
        let bimodal = TestDataGenerator::bimodal_distribution(20);
        assert_eq!(bimodal.len(), 20);

        // Test fee spike
        let spike = TestDataGenerator::fee_spike(50, 10);
        assert_eq!(spike.len(), 60);

        // Test graduated fees
        let graduated = TestDataGenerator::graduated_fees(30);
        assert_eq!(graduated.len(), 30);

        Ok(())
    }
}
