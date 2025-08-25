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
            total_time_span += block_duration;

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

    // ===== KOTLIN PARITY TESTS =====
    // These tests match InflowCalculatorTest from the Kotlin implementation

    #[test]
    fn parity_calculate_inflows_empty_snapshot_list() {
        // Matches Kotlin: "test calculateInflows with empty snapshot list"
        let inflows = InflowCalculator::calculate_inflows(&[], Duration::minutes(10));
        
        assert_eq!(inflows.len(), BUCKET_MAX as usize + 1);
        assert_eq!(inflows.sum(), 0.0);
    }

    #[test]
    fn parity_calculate_inflows_single_block_snapshots() {
        // Matches Kotlin: "test calculateInflows with single block snapshots"
        let base_time = Utc::now();
        
        // Create buckets with uniform weights
        let mut buckets1 = Array1::zeros(BUCKET_MAX as usize + 1);
        let mut buckets2 = Array1::zeros(BUCKET_MAX as usize + 1);
        buckets1.fill(1000.0);
        buckets2.fill(2000.0);
        
        let snapshots = vec![
            SnapshotArray::new(base_time, 100, buckets1),
            SnapshotArray::new(base_time + Duration::seconds(300), 100, buckets2),
        ];
        
        let inflows = InflowCalculator::calculate_inflows(&snapshots, Duration::minutes(10));
        
        // Each bucket increased by 1000, over 5 minutes
        // Normalized to 10 minutes, should be 2000 per bucket
        assert_eq!(inflows.len(), BUCKET_MAX as usize + 1);
        assert_eq!(inflows[0], 2000.0);
    }

    #[test]
    fn parity_calculate_inflows_consistent_rate() {
        // Matches Kotlin: "test calculateInflows with consistent inflow rate"
        let base_time = Utc::now();
        
        // Create snapshots with consistent inflow rate across all buckets
        let mut buckets1 = Array1::zeros(BUCKET_MAX as usize + 1);
        let mut buckets2 = Array1::zeros(BUCKET_MAX as usize + 1);
        let mut buckets3 = Array1::zeros(BUCKET_MAX as usize + 1);
        buckets1.fill(1_000_000.0);
        buckets2.fill(2_000_000.0);
        buckets3.fill(3_000_000.0);
        
        let snapshots = vec![
            SnapshotArray::new(base_time, 100, buckets1),
            SnapshotArray::new(base_time + Duration::seconds(300), 100, buckets2),
            SnapshotArray::new(base_time + Duration::seconds(600), 100, buckets3),
        ];
        
        let inflows = InflowCalculator::calculate_inflows(&snapshots, Duration::minutes(10));
        
        // Each bucket increased by 1M every 5 minutes
        // Normalized to 10 minutes, should be 2M per bucket
        assert_eq!(inflows.len(), BUCKET_MAX as usize + 1);
        assert_eq!(inflows[0], 2_000_000.0);
        assert_eq!(inflows[BUCKET_MAX as usize], 2_000_000.0);
    }

    #[test]
    fn parity_calculate_inflows_different_rates_per_bucket() {
        // Matches Kotlin: "test calculateInflows with different rates per bucket"
        let base_time = Utc::now();
        
        // Create snapshots with different inflow rates for different buckets
        let mut buckets1 = Array1::zeros(BUCKET_MAX as usize + 1);
        buckets1[0] = 1_000_000.0;
        buckets1[1] = 2_000_000.0;
        buckets1[2] = 3_000_000.0;
        
        let mut buckets2 = Array1::zeros(BUCKET_MAX as usize + 1);
        buckets2[0] = 2_000_000.0;
        buckets2[1] = 4_000_000.0;
        buckets2[2] = 6_000_000.0;
        
        let snapshots = vec![
            SnapshotArray::new(base_time, 100, buckets1),
            SnapshotArray::new(base_time + Duration::seconds(300), 100, buckets2),
        ];
        
        let inflows = InflowCalculator::calculate_inflows(&snapshots, Duration::minutes(10));
        
        // Over 5 minutes:
        // Bucket 0 increased by 1M -> 2M per 10 minutes
        // Bucket 1 increased by 2M -> 4M per 10 minutes
        // Bucket 2 increased by 3M -> 6M per 10 minutes
        assert_eq!(inflows.len(), BUCKET_MAX as usize + 1);
        assert_eq!(inflows[0], 2_000_000.0);
        assert_eq!(inflows[1], 4_000_000.0);
        assert_eq!(inflows[2], 6_000_000.0);
        assert_eq!(inflows[3], 0.0);
    }

    #[test]
    fn parity_calculate_inflows_first_last_snapshot_per_block() {
        // Matches Kotlin: "test calculateInflows considers only first and last snapshot per block height"
        let base_time = Utc::now();
        
        // Create snapshots for the same block height with fluctuating values
        let mut buckets1 = Array1::zeros(BUCKET_MAX as usize + 1);
        let mut buckets2 = Array1::zeros(BUCKET_MAX as usize + 1);
        let mut buckets3 = Array1::zeros(BUCKET_MAX as usize + 1);
        buckets1.fill(1000.0);
        buckets2.fill(500.0);  // Dip should be ignored
        buckets3.fill(2000.0);
        
        let snapshots = vec![
            SnapshotArray::new(base_time, 100, buckets1),
            SnapshotArray::new(base_time + Duration::seconds(100), 100, buckets2),
            SnapshotArray::new(base_time + Duration::seconds(300), 100, buckets3),
        ];
        
        let inflows = InflowCalculator::calculate_inflows(&snapshots, Duration::minutes(10));
        
        // Should only consider the difference between first (1000) and last (2000) snapshots
        // Over 5 minutes: increased by 1000 -> normalized to 2000 per 10 minutes
        assert_eq!(inflows.len(), BUCKET_MAX as usize + 1);
        assert_eq!(inflows[0], 2000.0);
    }

    #[test]
    fn parity_calculate_inflows_multiple_block_heights() {
        // Matches Kotlin: "test calculateInflows handles multiple block heights"
        let base_time = Utc::now();
        
        let mut buckets_100_1 = Array1::zeros(BUCKET_MAX as usize + 1);
        let mut buckets_100_2 = Array1::zeros(BUCKET_MAX as usize + 1);
        let mut buckets_100_3 = Array1::zeros(BUCKET_MAX as usize + 1);
        let mut buckets_101_1 = Array1::zeros(BUCKET_MAX as usize + 1);
        let mut buckets_101_2 = Array1::zeros(BUCKET_MAX as usize + 1);
        let mut buckets_101_3 = Array1::zeros(BUCKET_MAX as usize + 1);
        
        buckets_100_1.fill(1000.0);
        buckets_100_2.fill(500.0);   // Should be ignored
        buckets_100_3.fill(2000.0);
        buckets_101_1.fill(2000.0);
        buckets_101_2.fill(1500.0);  // Should be ignored
        buckets_101_3.fill(3000.0);
        
        let snapshots = vec![
            // Block 100
            SnapshotArray::new(base_time, 100, buckets_100_1),
            SnapshotArray::new(base_time + Duration::seconds(100), 100, buckets_100_2),
            SnapshotArray::new(base_time + Duration::seconds(200), 100, buckets_100_3),
            // Block 101
            SnapshotArray::new(base_time + Duration::seconds(300), 101, buckets_101_1),
            SnapshotArray::new(base_time + Duration::seconds(400), 101, buckets_101_2),
            SnapshotArray::new(base_time + Duration::seconds(500), 101, buckets_101_3),
        ];
        
        let inflows = InflowCalculator::calculate_inflows(&snapshots, Duration::minutes(10));
        
        // Block 100: +1000 over 200s
        // Block 101: +1000 over 200s
        // Total: +2000 over 400s = +3000 per 600s (10 minutes)
        assert_eq!(inflows.len(), BUCKET_MAX as usize + 1);
        assert_eq!(inflows[0], 3000.0);
    }
}
