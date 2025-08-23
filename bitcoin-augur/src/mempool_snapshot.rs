use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::mempool_transaction::MempoolTransaction;

/// Represents a snapshot of the Bitcoin mempool at a specific point in time.
///
/// The snapshot contains transactions grouped into buckets by fee rate,
/// along with metadata about when the snapshot was taken and at what block height.
///
/// # Example
/// ```
/// use bitcoin_augur::{MempoolSnapshot, MempoolTransaction};
/// use chrono::Utc;
///
/// let transactions = vec![
///     MempoolTransaction::new(400, 1000),  // 10 sat/vB
///     MempoolTransaction::new(600, 1200),  // 8 sat/vB
/// ];
///
/// let snapshot = MempoolSnapshot::from_transactions(
///     transactions,
///     850000,
///     Utc::now(),
/// );
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolSnapshot {
    /// The Bitcoin block height when this snapshot was taken
    pub block_height: u32,
    
    /// When this snapshot was taken
    pub timestamp: DateTime<Utc>,
    
    /// Map of fee rate bucket indices to total transaction weight
    /// The key is the bucket index (calculated logarithmically)
    /// The value is the total weight in that bucket
    pub bucketed_weights: BTreeMap<i32, u64>,
}

impl MempoolSnapshot {
    /// Creates a new mempool snapshot.
    pub fn new(
        block_height: u32,
        timestamp: DateTime<Utc>,
        bucketed_weights: BTreeMap<i32, u64>,
    ) -> Self {
        Self {
            block_height,
            timestamp,
            bucketed_weights,
        }
    }
    
    /// Creates a mempool snapshot from a list of mempool transactions.
    ///
    /// This method processes the raw transactions, buckets them by fee rate,
    /// and creates a snapshot that can be used for fee estimation.
    ///
    /// # Arguments
    /// * `transactions` - List of mempool transactions
    /// * `block_height` - Current block height
    /// * `timestamp` - When the snapshot is taken
    pub fn from_transactions(
        transactions: Vec<MempoolTransaction>,
        block_height: u32,
        timestamp: DateTime<Utc>,
    ) -> Self {
        let bucketed_weights = crate::internal::bucket_creator::create_fee_rate_buckets(&transactions);
        
        Self {
            block_height,
            timestamp,
            bucketed_weights,
        }
    }
    
    /// Creates an empty mempool snapshot.
    ///
    /// This can be useful for testing or when no mempool data is available.
    pub fn empty(block_height: u32, timestamp: DateTime<Utc>) -> Self {
        Self {
            block_height,
            timestamp,
            bucketed_weights: BTreeMap::new(),
        }
    }
    
    /// Returns the total weight across all buckets.
    pub fn total_weight(&self) -> u64 {
        self.bucketed_weights.values().sum()
    }
    
    /// Returns the number of fee rate buckets.
    pub fn bucket_count(&self) -> usize {
        self.bucketed_weights.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_empty_snapshot() {
        let snapshot = MempoolSnapshot::empty(850000, Utc::now());
        assert_eq!(snapshot.block_height, 850000);
        assert_eq!(snapshot.total_weight(), 0);
        assert_eq!(snapshot.bucket_count(), 0);
    }
    
    #[test]
    fn test_snapshot_metadata() {
        let now = Utc::now();
        let mut buckets = BTreeMap::new();
        buckets.insert(100, 1000);
        buckets.insert(200, 2000);
        
        let snapshot = MempoolSnapshot::new(850000, now, buckets);
        
        assert_eq!(snapshot.block_height, 850000);
        assert_eq!(snapshot.timestamp, now);
        assert_eq!(snapshot.total_weight(), 3000);
        assert_eq!(snapshot.bucket_count(), 2);
    }
}