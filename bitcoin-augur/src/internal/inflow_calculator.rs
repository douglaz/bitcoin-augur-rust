use chrono::Duration;
use ndarray::Array1;

use crate::internal::{snapshot_array::SnapshotArray, BUCKET_MAX};

/// Calculates transaction inflow rates for different fee rate buckets.
///
/// This is used to simulate new transactions entering the mempool
/// during the time period being estimated.
pub(crate) struct InflowCalculator;

impl InflowCalculator {
    /// Calculates inflow rates based on historical snapshots.
    ///
    /// # Arguments
    /// * `snapshots` - List of mempool snapshots as arrays
    /// * `timeframe` - Duration to consider for inflow calculation
    ///
    /// # Returns
    /// Array of inflow rates by fee rate bucket, normalized to 10 minutes
    pub fn calculate_inflows(snapshots: &[SnapshotArray], timeframe: Duration) -> Array1<f64> {
        if snapshots.is_empty() {
            return Array1::zeros(BUCKET_MAX as usize + 1);
        }

        // Sort snapshots by timestamp
        let mut ordered_snapshots = snapshots.to_vec();
        ordered_snapshots.sort_by_key(|s| s.timestamp);

        let end_time = ordered_snapshots.last().unwrap().timestamp;
        let start_time = end_time - timeframe;

        // Filter snapshots within timeframe
        let relevant_snapshots: Vec<_> = ordered_snapshots
            .iter()
            .filter(|s| s.timestamp >= start_time && s.timestamp <= end_time)
            .collect();

        if relevant_snapshots.is_empty() {
            return Array1::zeros(BUCKET_MAX as usize + 1);
        }

        let mut inflows = Array1::zeros(BUCKET_MAX as usize + 1);

        // Group snapshots by block height
        let mut snapshots_by_block: std::collections::BTreeMap<u32, Vec<&SnapshotArray>> =
            std::collections::BTreeMap::new();

        for snapshot in &relevant_snapshots {
            snapshots_by_block
                .entry(snapshot.block_height)
                .or_default()
                .push(snapshot);
        }

        // Calculate total time span for normalization
        let mut total_time_span = Duration::zero();

        // For each block, calculate inflows by comparing first and last snapshot
        for (_, block_snapshots) in snapshots_by_block {
            if block_snapshots.len() < 2 {
                continue; // Need at least 2 snapshots to calculate delta
            }

            let first_snapshot = block_snapshots.first().unwrap();
            let last_snapshot = block_snapshots.last().unwrap();

            // Add the duration between first and last snapshot of this block
            let block_duration = last_snapshot.timestamp - first_snapshot.timestamp;
            total_time_span = total_time_span + block_duration;

            // Calculate positive differences (inflows) between buckets
            let delta = &last_snapshot.buckets - &first_snapshot.buckets;

            // Only keep positive values (inflows)
            for i in 0..delta.len() {
                if delta[i] > 0.0 {
                    inflows[i] += delta[i];
                }
            }
        }

        // Normalize inflows to 10 minutes
        if total_time_span.num_seconds() > 0 {
            let ten_minutes = Duration::minutes(10);
            let normalization_factor =
                ten_minutes.num_seconds() as f64 / total_time_span.num_seconds() as f64;
            inflows *= normalization_factor;
        }

        inflows
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_snapshot(
        block_height: u32,
        seconds_offset: i64,
        bucket_weights: Vec<(usize, f64)>,
    ) -> SnapshotArray {
        let mut buckets = Array1::zeros(BUCKET_MAX as usize + 1);
        for (index, weight) in bucket_weights {
            buckets[index] = weight;
        }

        SnapshotArray::new(
            Utc::now() + Duration::seconds(seconds_offset),
            block_height,
            buckets,
        )
    }

    #[test]
    fn test_empty_snapshots() {
        let inflows = InflowCalculator::calculate_inflows(&[], Duration::hours(24));

        assert_eq!(inflows.len(), BUCKET_MAX as usize + 1);
        assert_eq!(inflows.sum(), 0.0);
    }

    #[test]
    fn test_single_block_inflow() {
        let snapshots = vec![
            create_test_snapshot(100, 0, vec![(10, 1000.0), (20, 2000.0)]),
            create_test_snapshot(100, 60, vec![(10, 1500.0), (20, 2500.0), (30, 500.0)]),
        ];

        let inflows = InflowCalculator::calculate_inflows(&snapshots, Duration::hours(1));

        // Check that inflows were calculated for increased weights
        assert_eq!(inflows[10], 500.0 * 10.0); // 500 increase, normalized to 10 min
        assert_eq!(inflows[20], 500.0 * 10.0); // 500 increase
        assert_eq!(inflows[30], 500.0 * 10.0); // 500 new bucket
    }

    #[test]
    fn test_negative_changes_ignored() {
        let snapshots = vec![
            create_test_snapshot(100, 0, vec![(10, 2000.0)]),
            create_test_snapshot(100, 60, vec![(10, 1000.0)]), // Decrease
        ];

        let inflows = InflowCalculator::calculate_inflows(&snapshots, Duration::hours(1));

        // Negative changes should be ignored (set to 0)
        assert_eq!(inflows[10], 0.0);
    }
}
