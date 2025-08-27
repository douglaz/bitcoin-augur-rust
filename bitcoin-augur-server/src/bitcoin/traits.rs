use async_trait::async_trait;
use bitcoin_augur::MempoolTransaction;

use super::RpcError;

/// Trait for Bitcoin RPC operations
#[async_trait]
pub trait BitcoinRpc: Send + Sync {
    /// Test connection to Bitcoin node
    async fn test_connection(&self) -> Result<(), RpcError>;

    /// Get current block height and mempool transactions
    async fn get_height_and_mempool(&self) -> Result<(u32, Vec<MempoolTransaction>), RpcError>;
}

/// Wrapper enum for real or mock client
pub enum BitcoinClient {
    Real(super::BitcoinRpcClient),
    Mock(super::MockBitcoinClient),
}

#[async_trait]
impl BitcoinRpc for BitcoinClient {
    async fn test_connection(&self) -> Result<(), RpcError> {
        match self {
            BitcoinClient::Real(client) => client.test_connection().await,
            BitcoinClient::Mock(client) => client.test_connection().await,
        }
    }

    async fn get_height_and_mempool(&self) -> Result<(u32, Vec<MempoolTransaction>), RpcError> {
        match self {
            BitcoinClient::Real(client) => client.get_height_and_mempool().await,
            BitcoinClient::Mock(client) => client.get_height_and_mempool().await,
        }
    }
}
