use bitcoin_augur::MempoolTransaction;
use base64::Engine;
use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
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
    RpcError { code: i32, message: String },
    
    #[error("Invalid response format")]
    InvalidResponse,
    
    #[error("Missing required field: {0}")]
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
        let auth = base64::engine::general_purpose::STANDARD.encode(
            format!("{}:{}", config.username, config.password)
        );
        
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
        let response = self.client
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
            blockchain_result.result
                .as_ref()
                .ok_or(RpcError::InvalidResponse)?
                .clone()
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
        
        let mempool_data = mempool_result.result
            .as_ref()
            .ok_or(RpcError::InvalidResponse)?
            .as_object()
            .ok_or(RpcError::InvalidResponse)?;
        
        let mut transactions = Vec::new();
        
        for (_txid, entry_value) in mempool_data {
            if let Ok(entry) = serde_json::from_value::<MempoolEntry>(entry_value.clone()) {
                // Use weight if available, otherwise calculate from vsize
                let weight = entry.weight.or_else(|| entry.vsize.map(|v| v * 4))
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
        
        let response = self.client
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
    
    #[test]
    fn test_config_creation() {
        let config = BitcoinRpcConfig {
            url: "http://localhost:8332".to_string(),
            username: "user".to_string(),
            password: "pass".to_string(),
        };
        
        let client = BitcoinRpcClient::new(config.clone());
        assert_eq!(client.config.url, "http://localhost:8332");
    }
}