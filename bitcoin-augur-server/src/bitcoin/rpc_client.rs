use base64::Engine;
use bitcoin_augur::MempoolTransaction;
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;
use tracing::{debug, error, info};

/// Bitcoin RPC configuration
#[derive(Debug, Clone)]
pub struct BitcoinRpcConfig {
    pub url: String,
    pub username: String,
    pub password: String,
}

/// Bitcoin RPC error types
#[derive(Error, Debug)]
pub enum RpcError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("JSON parsing failed: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("RPC error: {message}")]
    #[allow(clippy::enum_variant_names)]
    RpcError { code: i32, message: String },

    #[error("Invalid response format")]
    InvalidResponse,

    #[error("Missing required field: {0}")]
    #[allow(dead_code)]
    MissingField(String),
}

/// Bitcoin RPC client for fetching mempool data
pub struct BitcoinRpcClient {
    client: Client,
    config: BitcoinRpcConfig,
    auth_header: String,
}

#[derive(Serialize)]
struct RpcRequest {
    jsonrpc: &'static str,
    id: String,
    method: String,
    params: Vec<Value>,
}

#[derive(Deserialize)]
struct RpcResponse {
    result: Option<Value>,
    error: Option<RpcErrorResponse>,
    #[allow(dead_code)]
    id: String,
}

#[derive(Deserialize)]
struct RpcErrorResponse {
    code: i32,
    message: String,
}

#[derive(Deserialize)]
struct BlockchainInfo {
    blocks: u32,
    #[allow(dead_code)]
    #[serde(rename = "bestblockhash")]
    best_block_hash: String,
}

#[derive(Deserialize)]
struct MempoolEntry {
    #[serde(rename = "vsize")]
    vsize: Option<u64>,
    weight: Option<u64>,
    fees: MempoolFees,
}

#[derive(Deserialize)]
struct MempoolFees {
    base: f64,
}

impl BitcoinRpcClient {
    /// Creates a new Bitcoin RPC client
    pub fn new(config: BitcoinRpcConfig) -> Self {
        let auth = base64::engine::general_purpose::STANDARD
            .encode(format!("{}:{}", config.username, config.password));

        Self {
            client: Client::new(),
            auth_header: format!("Basic {}", auth),
            config,
        }
    }

    /// Gets current blockchain height and mempool transactions
    pub async fn get_height_and_mempool(&self) -> Result<(u32, Vec<MempoolTransaction>), RpcError> {
        info!("Fetching blockchain height and mempool data");

        // Create batch RPC request
        let batch_request = vec![
            RpcRequest {
                jsonrpc: "1.0",
                id: "blockchain-info".to_string(),
                method: "getblockchaininfo".to_string(),
                params: vec![],
            },
            RpcRequest {
                jsonrpc: "1.0",
                id: "mempool".to_string(),
                method: "getrawmempool".to_string(),
                params: vec![json!(true)], // verbose=true for detailed info
            },
        ];

        // Send batch request
        let response = self
            .client
            .post(&self.config.url)
            .header(header::AUTHORIZATION, &self.auth_header)
            .header(header::CONTENT_TYPE, "application/json")
            .json(&batch_request)
            .send()
            .await?;

        if !response.status().is_success() {
            error!("RPC request failed with status: {}", response.status());
            return Err(RpcError::InvalidResponse);
        }

        let results: Vec<RpcResponse> = response.json().await?;

        if results.len() != 2 {
            return Err(RpcError::InvalidResponse);
        }

        // Parse blockchain info
        let blockchain_result = &results[0];
        if let Some(ref error) = blockchain_result.error {
            return Err(RpcError::RpcError {
                code: error.code,
                message: error.message.clone(),
            });
        }

        let blockchain_info: BlockchainInfo = serde_json::from_value(
            blockchain_result
                .result
                .as_ref()
                .ok_or(RpcError::InvalidResponse)?
                .clone(),
        )?;

        debug!("Current blockchain height: {}", blockchain_info.blocks);

        // Parse mempool transactions
        let mempool_result = &results[1];
        if let Some(ref error) = mempool_result.error {
            return Err(RpcError::RpcError {
                code: error.code,
                message: error.message.clone(),
            });
        }

        let mempool_data = mempool_result
            .result
            .as_ref()
            .ok_or(RpcError::InvalidResponse)?
            .as_object()
            .ok_or(RpcError::InvalidResponse)?;

        let mut transactions = Vec::new();

        for (_txid, entry_value) in mempool_data {
            if let Ok(entry) = serde_json::from_value::<MempoolEntry>(entry_value.clone()) {
                // Use weight if available, otherwise calculate from vsize
                let weight = entry
                    .weight
                    .or_else(|| entry.vsize.map(|v| v * 4))
                    .unwrap_or(0);

                if weight > 0 {
                    // Convert BTC to satoshis
                    let fee_sats = (entry.fees.base * 100_000_000.0) as u64;

                    transactions.push(MempoolTransaction::new(weight, fee_sats));
                }
            }
        }

        info!("Fetched {} mempool transactions", transactions.len());

        Ok((blockchain_info.blocks, transactions))
    }

    /// Tests the RPC connection
    pub async fn test_connection(&self) -> Result<(), RpcError> {
        debug!("Testing Bitcoin RPC connection");

        let request = RpcRequest {
            jsonrpc: "1.0",
            id: "test".to_string(),
            method: "getblockcount".to_string(),
            params: vec![],
        };

        let response = self
            .client
            .post(&self.config.url)
            .header(header::AUTHORIZATION, &self.auth_header)
            .header(header::CONTENT_TYPE, "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            error!("Connection test failed with status: {}", response.status());
            return Err(RpcError::InvalidResponse);
        }

        let result: RpcResponse = response.json().await?;

        if let Some(error) = result.error {
            return Err(RpcError::RpcError {
                code: error.code,
                message: error.message,
            });
        }

        if result.result.is_none() {
            return Err(RpcError::InvalidResponse);
        }

        info!("Bitcoin RPC connection successful");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn test_config_creation() {
        let config = BitcoinRpcConfig {
            url: "http://localhost:8332".to_string(),
            username: "user".to_string(),
            password: "pass".to_string(),
        };

        let client = BitcoinRpcClient::new(config.clone());
        assert_eq!(client.config.url, "http://localhost:8332");

        // Check auth header encoding
        let expected_auth = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode("user:pass")
        );
        assert_eq!(client.auth_header, expected_auth);
    }

    #[tokio::test]
    async fn test_successful_connection() {
        let mock_server = MockServer::start().await;

        let config = BitcoinRpcConfig {
            url: mock_server.uri(),
            username: "test".to_string(),
            password: "pass".to_string(),
        };

        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("authorization", "Basic dGVzdDpwYXNz"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "result": 850000,
                "error": null,
                "id": "test"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = BitcoinRpcClient::new(config);
        let result = client.test_connection().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_connection_auth_failure() {
        let mock_server = MockServer::start().await;

        let config = BitcoinRpcConfig {
            url: mock_server.uri(),
            username: "test".to_string(),
            password: "pass".to_string(),
        };

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(401))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = BitcoinRpcClient::new(config);
        let result = client.test_connection().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_rpc_error_response() {
        let mock_server = MockServer::start().await;

        let config = BitcoinRpcConfig {
            url: mock_server.uri(),
            username: "test".to_string(),
            password: "pass".to_string(),
        };

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "result": null,
                "error": {
                    "code": -28,
                    "message": "Loading block index..."
                },
                "id": "test"
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = BitcoinRpcClient::new(config);
        let result = client.test_connection().await;

        match result {
            Err(RpcError::RpcError { code, message }) => {
                assert_eq!(code, -28);
                assert_eq!(message, "Loading block index...");
            }
            _ => panic!("Expected RpcError"),
        }
    }

    #[tokio::test]
    async fn test_get_height_and_mempool_success() {
        let mock_server = MockServer::start().await;

        let config = BitcoinRpcConfig {
            url: mock_server.uri(),
            username: "test".to_string(),
            password: "pass".to_string(),
        };

        // Mock batch response
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_json(json!([
                    {
                        "result": {
                            "blocks": 850000,
                            "bestblockhash": "00000000000000000002a7c4c1e48d76c5a37902165a270156b7a8d72728a054"
                        },
                        "error": null,
                        "id": "blockchain-info"
                    },
                    {
                        "result": {
                            "tx1": {
                                "vsize": 250,
                                "weight": 1000,
                                "fees": {
                                    "base": 0.00001000
                                }
                            },
                            "tx2": {
                                "vsize": 150,
                                "fees": {
                                    "base": 0.00002000
                                }
                            }
                        },
                        "error": null,
                        "id": "mempool"
                    }
                ])))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = BitcoinRpcClient::new(config);
        let result = client.get_height_and_mempool().await.unwrap();

        assert_eq!(result.0, 850000);
        assert_eq!(result.1.len(), 2);

        // Check first transaction
        assert_eq!(result.1[0].weight, 1000);
        assert_eq!(result.1[0].fee, 1000); // 0.00001 BTC = 1000 sats

        // Check second transaction (weight calculated from vsize)
        assert_eq!(result.1[1].weight, 600); // 150 * 4
        assert_eq!(result.1[1].fee, 2000); // 0.00002 BTC = 2000 sats
    }

    #[tokio::test]
    async fn test_get_height_and_mempool_empty_mempool() {
        let mock_server = MockServer::start().await;

        let config = BitcoinRpcConfig {
            url: mock_server.uri(),
            username: "test".to_string(),
            password: "pass".to_string(),
        };

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {
                    "result": {
                        "blocks": 850000,
                        "bestblockhash": "hash"
                    },
                    "error": null,
                    "id": "blockchain-info"
                },
                {
                    "result": {},
                    "error": null,
                    "id": "mempool"
                }
            ])))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = BitcoinRpcClient::new(config);
        let result = client.get_height_and_mempool().await.unwrap();

        assert_eq!(result.0, 850000);
        assert_eq!(result.1.len(), 0);
    }

    #[tokio::test]
    async fn test_get_height_and_mempool_malformed_response() {
        let mock_server = MockServer::start().await;

        let config = BitcoinRpcConfig {
            url: mock_server.uri(),
            username: "test".to_string(),
            password: "pass".to_string(),
        };

        // Return only one response instead of two
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {
                    "result": {
                        "blocks": 850000,
                        "bestblockhash": "hash"
                    },
                    "error": null,
                    "id": "blockchain-info"
                }
            ])))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = BitcoinRpcClient::new(config);
        let result = client.get_height_and_mempool().await;

        match result {
            Err(RpcError::InvalidResponse) => {}
            _ => panic!("Expected InvalidResponse error"),
        }
    }

    #[tokio::test]
    async fn test_network_error() {
        // Use an invalid URL that will cause a network error
        let config = BitcoinRpcConfig {
            url: "http://invalid-host-that-does-not-exist:8332".to_string(),
            username: "test".to_string(),
            password: "pass".to_string(),
        };

        let client = BitcoinRpcClient::new(config);
        let result = client.test_connection().await;

        match result {
            Err(RpcError::HttpError(_)) => {}
            _ => panic!("Expected HttpError"),
        }
    }

    #[tokio::test]
    async fn test_invalid_json_response() {
        let mock_server = MockServer::start().await;

        let config = BitcoinRpcConfig {
            url: mock_server.uri(),
            username: "test".to_string(),
            password: "pass".to_string(),
        };

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_string("invalid json"))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = BitcoinRpcClient::new(config);
        let result = client.test_connection().await;

        match result {
            Err(RpcError::JsonError(_)) => {}
            _ => panic!("Expected JsonError"),
        }
    }

    #[tokio::test]
    async fn test_transaction_with_zero_weight() {
        let mock_server = MockServer::start().await;

        let config = BitcoinRpcConfig {
            url: mock_server.uri(),
            username: "test".to_string(),
            password: "pass".to_string(),
        };

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                {
                    "result": {
                        "blocks": 850000,
                        "bestblockhash": "hash"
                    },
                    "error": null,
                    "id": "blockchain-info"
                },
                {
                    "result": {
                        "tx1": {
                            "weight": 0,
                            "fees": {
                                "base": 0.00001000
                            }
                        },
                        "tx2": {
                            "weight": 1000,
                            "fees": {
                                "base": 0.00002000
                            }
                        }
                    },
                    "error": null,
                    "id": "mempool"
                }
            ])))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = BitcoinRpcClient::new(config);
        let result = client.get_height_and_mempool().await.unwrap();

        // Transaction with zero weight should be filtered out
        assert_eq!(result.1.len(), 1);
        assert_eq!(result.1[0].weight, 1000);
    }
}
