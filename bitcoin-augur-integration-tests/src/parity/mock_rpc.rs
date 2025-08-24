use anyhow::Result;
use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{debug, info};

use super::test_data::{TestSnapshot, TestTransaction};

pub struct MockBitcoinRpc {
    mempool: Arc<RwLock<Vec<TestTransaction>>>,
    port: u16,
}

impl MockBitcoinRpc {
    pub fn new(port: u16) -> Self {
        // Start with some initial transactions for testing
        let initial_txs = vec![
            TestTransaction {
                weight: 1000,
                fee: 10000,
                fee_rate: 10.0, // 10 sat/vB
            },
            TestTransaction {
                weight: 2000,
                fee: 40000,
                fee_rate: 20.0, // 20 sat/vB
            },
            TestTransaction {
                weight: 1500,
                fee: 22500,
                fee_rate: 15.0, // 15 sat/vB
            },
        ];

        Self {
            mempool: Arc::new(RwLock::new(initial_txs)),
            port,
        }
    }

    pub async fn start(&self) -> Result<()> {
        let mempool = self.mempool.clone();

        let app = Router::new()
            .route("/", post(handle_rpc))
            .with_state(mempool);

        let port = self.port;
        let addr = format!("127.0.0.1:{port}");
        info!("Mock Bitcoin RPC server listening on {addr}");

        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }

    #[allow(dead_code)]
    pub fn set_mempool(&self, transactions: Vec<TestTransaction>) {
        *self.mempool.write().unwrap() = transactions;
    }

    #[allow(dead_code)]
    pub fn get_mempool(&self) -> Vec<TestTransaction> {
        self.mempool.read().unwrap().clone()
    }

    #[allow(dead_code)]
    pub fn inject_snapshot(&self, snapshot: &TestSnapshot) {
        let mut mempool = self.mempool.write().unwrap();
        mempool.clear();
        mempool.extend(snapshot.transactions.clone());
    }

    #[allow(dead_code)]
    pub fn clear_mempool(&self) {
        self.mempool.write().unwrap().clear();
    }

    #[allow(dead_code)]
    pub fn add_transaction(&self, tx: TestTransaction) {
        self.mempool.write().unwrap().push(tx);
    }
}

#[derive(Debug, Deserialize)]
struct RpcRequest {
    method: String,
    params: Option<Vec<Value>>,
    id: Option<Value>,
}

#[derive(Debug, Serialize)]
struct RpcResponse {
    result: Option<Value>,
    error: Option<RpcError>,
    id: Option<Value>,
}

#[derive(Debug, Serialize)]
struct RpcError {
    code: i32,
    message: String,
}

async fn handle_rpc(
    State(mempool): State<Arc<RwLock<Vec<TestTransaction>>>>,
    Json(req): Json<RpcRequest>,
) -> (StatusCode, Json<RpcResponse>) {
    let method = &req.method;
    debug!("Mock RPC received method: {method}");

    let response = match req.method.as_str() {
        "getrawmempool" => {
            // Return transaction IDs (simplified - just use index as txid)
            let mempool_data = mempool.read().unwrap();
            let verbose = req
                .params
                .as_ref()
                .and_then(|p| p.first())
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if verbose {
                // Return detailed mempool info
                let mut entries = HashMap::new();
                for (idx, tx) in mempool_data.iter().enumerate() {
                    let txid = format!("tx{:064x}", idx);
                    entries.insert(
                        txid,
                        json!({
                            "size": tx.weight / 4,
                            "weight": tx.weight,
                            "fee": tx.fee as f64 / 100_000_000.0, // Convert to BTC
                            "modifiedfee": tx.fee as f64 / 100_000_000.0,
                            "time": 1234567890,
                            "height": 850000,
                            "descendantcount": 1,
                            "descendantsize": tx.weight / 4,
                            "descendantfees": tx.fee,
                            "ancestorcount": 1,
                            "ancestorsize": tx.weight / 4,
                            "ancestorfees": tx.fee,
                            "wtxid": format!("wtx{:064x}", idx),
                            "fees": {
                                "base": tx.fee as f64 / 100_000_000.0,
                                "modified": tx.fee as f64 / 100_000_000.0,
                                "ancestor": tx.fee as f64 / 100_000_000.0,
                                "descendant": tx.fee as f64 / 100_000_000.0,
                            },
                            "depends": [],
                            "spentby": [],
                            "bip125-replaceable": false,
                            "unbroadcast": false
                        }),
                    );
                }
                RpcResponse {
                    result: Some(serde_json::to_value(entries).unwrap()),
                    error: None,
                    id: req.id,
                }
            } else {
                // Just return txids
                let txids: Vec<String> = (0..mempool_data.len())
                    .map(|idx| format!("tx{:064x}", idx))
                    .collect();
                RpcResponse {
                    result: Some(serde_json::to_value(txids).unwrap()),
                    error: None,
                    id: req.id,
                }
            }
        }

        "getmempoolentry" => {
            // Return details for a specific transaction
            let txid = req
                .params
                .as_ref()
                .and_then(|p| p.first())
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // Extract index from txid (format: "tx{index:064x}")
            if let Some(idx_str) = txid.strip_prefix("tx") {
                if let Ok(idx) = usize::from_str_radix(idx_str, 16) {
                    let mempool_data = mempool.read().unwrap();
                    if let Some(tx) = mempool_data.get(idx) {
                        let entry = json!({
                            "size": tx.weight / 4,
                            "weight": tx.weight,
                            "fee": tx.fee as f64 / 100_000_000.0,
                            "modifiedfee": tx.fee as f64 / 100_000_000.0,
                            "time": 1234567890,
                            "height": 850000,
                            "descendantcount": 1,
                            "descendantsize": tx.weight / 4,
                            "descendantfees": tx.fee,
                            "ancestorcount": 1,
                            "ancestorsize": tx.weight / 4,
                            "ancestorfees": tx.fee,
                            "wtxid": format!("wtx{:064x}", idx),
                            "fees": {
                                "base": tx.fee as f64 / 100_000_000.0,
                                "modified": tx.fee as f64 / 100_000_000.0,
                                "ancestor": tx.fee as f64 / 100_000_000.0,
                                "descendant": tx.fee as f64 / 100_000_000.0,
                            },
                            "depends": [],
                            "spentby": [],
                            "bip125-replaceable": false,
                            "unbroadcast": false
                        });

                        RpcResponse {
                            result: Some(entry),
                            error: None,
                            id: req.id,
                        }
                    } else {
                        RpcResponse {
                            result: None,
                            error: Some(RpcError {
                                code: -5,
                                message: "Transaction not in mempool".to_string(),
                            }),
                            id: req.id,
                        }
                    }
                } else {
                    RpcResponse {
                        result: None,
                        error: Some(RpcError {
                            code: -8,
                            message: "Invalid transaction id".to_string(),
                        }),
                        id: req.id,
                    }
                }
            } else {
                RpcResponse {
                    result: None,
                    error: Some(RpcError {
                        code: -8,
                        message: "Invalid transaction id".to_string(),
                    }),
                    id: req.id,
                }
            }
        }

        "getblockcount" => RpcResponse {
            result: Some(json!(850000)),
            error: None,
            id: req.id,
        },

        "getblockchaininfo" => RpcResponse {
            result: Some(json!({
                "chain": "main",
                "blocks": 850000,
                "headers": 850000,
                "bestblockhash": "0000000000000000000000000000000000000000000000000000000000000000",
                "difficulty": 88103718325334.92,
                "time": 1234567890,
                "mediantime": 1234567890,
                "verificationprogress": 0.9999999,
                "initialblockdownload": false,
                "chainwork": "0000000000000000000000000000000000000000000000000000000000000000",
                "size_on_disk": 600000000000i64,
                "pruned": false,
                "warnings": ""
            })),
            error: None,
            id: req.id,
        },

        _ => {
            let method = &req.method;
            RpcResponse {
                result: None,
                error: Some(RpcError {
                    code: -32601,
                    message: format!("Method not found: {method}"),
                }),
                id: req.id,
            }
        }
    };

    (StatusCode::OK, Json(response))
}
