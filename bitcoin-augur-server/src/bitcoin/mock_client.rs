use super::RpcError;
use bitcoin_augur::MempoolTransaction;

/// Mock Bitcoin RPC client for testing
#[derive(Clone, Default)]
pub struct MockBitcoinClient;

impl MockBitcoinClient {
    pub fn new() -> Self {
        Self
    }

    /// Test connection (always succeeds in mock mode)
    pub async fn test_connection(&self) -> Result<(), RpcError> {
        Ok(())
    }

    /// Get current block height and mempool (returns mock data)
    pub async fn get_height_and_mempool(&self) -> Result<(u32, Vec<MempoolTransaction>), RpcError> {
        // Return mock block height and some simple transactions
        let transactions = vec![
            MempoolTransaction::new(2000, 2000), // 1 sat/vB
            MempoolTransaction::new(2000, 4000), // 2 sat/vB
            MempoolTransaction::new(2000, 6000), // 3 sat/vB
        ];
        Ok((850000, transactions))
    }
}
