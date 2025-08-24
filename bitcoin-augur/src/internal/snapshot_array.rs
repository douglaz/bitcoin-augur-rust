use chrono::{DateTime, Utc};
use ndarray::Array1;

use crate::internal::BUCKET_MAX;
use crate::mempool_snapshot::MempoolSnapshot;

/// Internal representation of mempool snapshot using ndarray for efficient calculations.
///
/// This mirrors the Kotlin implementation's MempoolSnapshotF64Array, providing
/// efficient numerical operations for fee estimation calculations.
#[derive(Debug, Clone)]
pub(crate) struct SnapshotArray {
    pub timestamp: DateTime<Utc>,
    pub block_height: u32,
    pub buckets: Array1<f64>,
}

impl SnapshotArray {
    /// Creates a new snapshot array.
    #[allow(dead_code)]
    pub fn new(timestamp: DateTime<Utc>, block_height: u32, buckets: Array1<f64>) -> Self {
        Self {
            timestamp,
            block_height,
            buckets,
        }
    }

    /// Converts a MempoolSnapshot to SnapshotArray for efficient calculations.
    ///
    /// The buckets are stored in reverse order to allow mining the highest
    /// fee rate transactions first during simulation.
    pub fn from_snapshot(snapshot: &MempoolSnapshot) -> Self {
        let mut fee_rate_buckets = Array1::zeros(BUCKET_MAX as usize + 1);

        for (&bucket, &weight) in &snapshot.bucketed_weights {
            // Remove buckets that are less than 0 (i.e. fee rates less than 1 sat/vB)
            if bucket >= 0 {
                // Insert in reverse order to mine highest fee rate buckets first
                let index = (BUCKET_MAX - bucket) as usize;
                fee_rate_buckets[index] = weight as f64;
            }
        }

        Self {
            timestamp: snapshot.timestamp,
            block_height: snapshot.block_height,
            buckets: fee_rate_buckets,
        }
    }

    /// Returns the total weight across all buckets.
    #[allow(dead_code)]
    pub fn total_weight(&self) -> f64 {
        self.buckets.sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_from_snapshot() {
        let mut bucketed_weights = BTreeMap::new();
        bucketed_weights.insert(100, 1000);
        bucketed_weights.insert(200, 2000);

        let snapshot = MempoolSnapshot::new(850000, Utc::now(), bucketed_weights);

        let array = SnapshotArray::from_snapshot(&snapshot);

        assert_eq!(array.block_height, 850000);
        assert_eq!(array.buckets.len(), BUCKET_MAX as usize + 1);

        // Check that weights are in reverse order
        assert_eq!(array.buckets[(BUCKET_MAX - 100) as usize], 1000.0);
        assert_eq!(array.buckets[(BUCKET_MAX - 200) as usize], 2000.0);
        assert_eq!(array.total_weight(), 3000.0);
    }

    #[test]
    fn test_negative_buckets_filtered() {
        let mut bucketed_weights = BTreeMap::new();
        bucketed_weights.insert(-10, 500); // Should be filtered out
        bucketed_weights.insert(50, 1500);

        let snapshot = MempoolSnapshot::new(850000, Utc::now(), bucketed_weights);

        let array = SnapshotArray::from_snapshot(&snapshot);

        // Only positive bucket should be included
        assert_eq!(array.buckets[(BUCKET_MAX - 50) as usize], 1500.0);
        assert_eq!(array.total_weight(), 1500.0);
    }
}
