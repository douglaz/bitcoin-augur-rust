//! Stress tests for Bitcoin Augur fee estimation
//!
//! These tests verify the system's behavior under extreme conditions:
//! - Large mempool sizes (up to 1M transactions)
//! - Rapid mempool changes
//! - Memory usage patterns
//! - Performance degradation

use bitcoin_augur::{FeeEstimator, MempoolSnapshot, MempoolTransaction};
use chrono::{Duration, Utc};
use std::time::Instant;

/// Generate a large mempool with specified characteristics
fn generate_large_mempool(
    num_transactions: usize,
    fee_range: (f64, f64),
) -> Vec<MempoolTransaction> {
    let mut transactions = Vec::with_capacity(num_transactions);
    let (min_fee, max_fee) = fee_range;

    for i in 0..num_transactions {
        // Generate varied fee rates using a somewhat realistic distribution
        // Most transactions at lower fees, fewer at higher fees
        let fee_percentile = (i as f64) / (num_transactions as f64);
        let fee_rate = min_fee + (max_fee - min_fee) * fee_percentile.powf(2.0);

        // Vary transaction weights realistically (250-2000 weight units)
        let weight = 250 + (i % 1750) as u64;
        let fee = (fee_rate * weight as f64 / 4.0) as u64;

        transactions.push(MempoolTransaction::new(weight, fee));
    }

    // Shuffle to avoid any ordering bias in tests
    use rand::rng;
    use rand::seq::SliceRandom;
    let mut rng = rng();
    transactions.shuffle(&mut rng);

    transactions
}

#[test]
#[ignore] // Run with: cargo test stress_tests -- --ignored
fn test_large_mempool_100k_transactions() {
    let transactions = generate_large_mempool(100_000, (1.0, 1000.0));
    let snapshot = MempoolSnapshot::from_transactions(transactions, 800_000, Utc::now());

    let estimator = FeeEstimator::new();
    let start = Instant::now();

    let result = estimator.calculate_estimates(&[snapshot], None);

    let duration = start.elapsed();

    assert!(result.is_ok(), "Should handle 100k transactions");
    assert!(
        duration.as_millis() < 1000,
        "Should complete within 1 second, took {}ms",
        duration.as_millis()
    );

    let estimate = result.unwrap();
    assert!(!estimate.estimates.is_empty(), "Should produce estimates");
}

#[test]
#[ignore] // Run with: cargo test stress_tests -- --ignored
fn test_very_large_mempool_500k_transactions() {
    let transactions = generate_large_mempool(500_000, (1.0, 2000.0));
    let snapshot = MempoolSnapshot::from_transactions(transactions, 800_000, Utc::now());

    let estimator = FeeEstimator::new();
    let start = Instant::now();

    let result = estimator.calculate_estimates(&[snapshot], None);

    let duration = start.elapsed();

    assert!(result.is_ok(), "Should handle 500k transactions");
    assert!(
        duration.as_secs() < 5,
        "Should complete within 5 seconds, took {}s",
        duration.as_secs()
    );
}

#[test]
#[ignore] // Run with: cargo test stress_tests -- --ignored
fn test_extreme_mempool_1m_transactions() {
    let transactions = generate_large_mempool(1_000_000, (1.0, 5000.0));
    let snapshot = MempoolSnapshot::from_transactions(transactions, 800_000, Utc::now());

    let estimator = FeeEstimator::new();
    let start = Instant::now();

    let result = estimator.calculate_estimates(&[snapshot], None);

    let duration = start.elapsed();

    assert!(result.is_ok(), "Should handle 1M transactions");
    assert!(
        duration.as_secs() < 10,
        "Should complete within 10 seconds, took {}s",
        duration.as_secs()
    );
}

#[test]
#[ignore] // Run with: cargo test stress_tests -- --ignored
fn test_many_snapshots_stress() {
    // Test with 1000 snapshots (about 7 days of data at 10-minute intervals)
    let num_snapshots = 1000;
    let mut snapshots = Vec::with_capacity(num_snapshots);
    let mut base_time = Utc::now() - Duration::days(7);

    for i in 0..num_snapshots {
        // Vary mempool size over time (5k to 50k transactions)
        let size = 5000 + (i * 45) % 45000;
        let transactions = generate_large_mempool(size, (1.0, 500.0));

        let snapshot =
            MempoolSnapshot::from_transactions(transactions, 800_000 + i as u32, base_time);

        snapshots.push(snapshot);
        base_time += Duration::minutes(10);
    }

    let estimator = FeeEstimator::new();
    let start = Instant::now();

    let result = estimator.calculate_estimates(&snapshots, None);

    let duration = start.elapsed();

    assert!(result.is_ok(), "Should handle 1000 snapshots");
    assert!(
        duration.as_secs() < 30,
        "Should complete within 30 seconds, took {}s",
        duration.as_secs()
    );
}

#[test]
fn test_rapid_mempool_changes() {
    // Simulate rapid mempool changes with vastly different sizes
    let sizes = [1000, 50000, 2000, 80000, 500, 100000, 3000];
    let mut snapshots = Vec::new();
    let mut base_time = Utc::now() - Duration::hours(1);

    for (i, &size) in sizes.iter().enumerate() {
        let transactions = generate_large_mempool(size, (1.0, 1000.0));
        let snapshot =
            MempoolSnapshot::from_transactions(transactions, 800_000 + i as u32, base_time);

        snapshots.push(snapshot);
        base_time += Duration::minutes(10);
    }

    let estimator = FeeEstimator::new();
    let result = estimator.calculate_estimates(&snapshots, None);

    assert!(result.is_ok(), "Should handle rapid size changes");

    let estimate = result.unwrap();
    // Verify estimates are still reasonable despite volatility
    for block_target in estimate.estimates.values() {
        for fee_rate in block_target.probabilities.values() {
            assert!(
                *fee_rate >= 1.0 && *fee_rate <= 10000.0,
                "Fee rate should be within reasonable bounds: {}",
                fee_rate
            );
        }
    }
}

#[test]
fn test_extreme_fee_rates() {
    // Test with extreme fee rate ranges
    let mut transactions = Vec::new();

    // Add some very low fee transactions
    for i in 0..1000 {
        let weight = 1000 + (i % 1000) as u64;
        let fee_rate = 0.1 + (i as f64) * 0.001; // 0.1 to 1.1 sat/vB
        let fee = (fee_rate * weight as f64 / 4.0) as u64;
        transactions.push(MempoolTransaction::new(weight, fee));
    }

    // Add some very high fee transactions
    for i in 0..100 {
        let weight = 500 + (i % 500) as u64;
        let fee_rate = 5000.0 + (i as f64) * 50.0; // 5000 to 10000 sat/vB
        let fee = (fee_rate * weight as f64 / 4.0) as u64;
        transactions.push(MempoolTransaction::new(weight, fee));
    }

    let snapshot = MempoolSnapshot::from_transactions(transactions, 800_000, Utc::now());

    let estimator = FeeEstimator::new();
    let result = estimator.calculate_estimates(&[snapshot], None);

    assert!(result.is_ok(), "Should handle extreme fee rate ranges");
}

#[test]
fn test_memory_efficiency_bucket_consolidation() {
    // Test that bucket consolidation keeps memory usage reasonable
    // even with many unique fee rates
    let mut transactions = Vec::new();

    // Create transactions with 100,000 unique fee rates
    for i in 0..100_000 {
        let weight = 1000;
        let fee_rate = 1.0 + (i as f64) * 0.01; // Each transaction has unique fee rate
        let fee = (fee_rate * weight as f64 / 4.0) as u64;
        transactions.push(MempoolTransaction::new(weight, fee));
    }

    let snapshot = MempoolSnapshot::from_transactions(transactions, 800_000, Utc::now());

    // The bucketing should consolidate these into far fewer buckets
    assert!(
        snapshot.bucket_count() < 10_000,
        "Should consolidate 100k unique rates into < 10k buckets, got {}",
        snapshot.bucket_count()
    );

    let estimator = FeeEstimator::new();
    let result = estimator.calculate_estimates(&[snapshot], None);
    assert!(
        result.is_ok(),
        "Should handle many unique fee rates efficiently"
    );
}

#[test]
#[ignore] // Run with: cargo test stress_tests -- --ignored
fn test_continuous_operation_simulation() {
    // Simulate continuous operation for extended period
    let mut snapshots = Vec::new();
    let mut base_time = Utc::now() - Duration::hours(24);
    let estimator = FeeEstimator::new();

    // Simulate 24 hours of operation with snapshots every 5 minutes
    for hour in 0..24 {
        for minute in (0..60).step_by(5) {
            // Vary mempool size based on time of day
            let time_factor =
                ((hour as f64 + minute as f64 / 60.0) * std::f64::consts::PI / 12.0).sin();
            let size = (20_000.0 + 30_000.0 * time_factor) as usize;

            let transactions = generate_large_mempool(size, (1.0, 500.0));
            let snapshot = MempoolSnapshot::from_transactions(
                transactions,
                800_000 + (hour * 12 + minute / 5) as u32,
                base_time,
            );

            snapshots.push(snapshot);
            base_time += Duration::minutes(5);

            // Keep only last 144 snapshots (12 hours) to simulate sliding window
            if snapshots.len() > 144 {
                snapshots.remove(0);
            }

            // Calculate estimates periodically
            if minute == 0 {
                let result = estimator.calculate_estimates(&snapshots, None);
                assert!(
                    result.is_ok(),
                    "Continuous operation should not fail at hour {}",
                    hour
                );
            }
        }
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;

    #[test]
    #[ignore] // Run with: cargo test stress_tests -- --ignored
    fn test_performance_degradation_with_size() {
        let sizes = vec![1000, 10_000, 50_000, 100_000, 200_000];
        let mut timings = Vec::new();
        let estimator = FeeEstimator::new();

        for size in sizes {
            let transactions = generate_large_mempool(size, (1.0, 1000.0));
            let snapshot = MempoolSnapshot::from_transactions(transactions, 800_000, Utc::now());

            let start = Instant::now();
            let _ = estimator.calculate_estimates(&[snapshot], None);
            let duration = start.elapsed();

            timings.push((size, duration));
            println!("Size: {}, Time: {:?}", size, duration);
        }

        // Check that performance degradation is roughly linear or better
        // (not exponential)
        for i in 1..timings.len() {
            let (prev_size, prev_time) = timings[i - 1];
            let (curr_size, curr_time) = timings[i];

            let size_ratio = curr_size as f64 / prev_size as f64;
            let time_ratio = curr_time.as_millis() as f64 / prev_time.as_millis().max(1) as f64;

            // Allow up to 2x the linear scaling as acceptable
            assert!(
                time_ratio < size_ratio * 2.0,
                "Performance degradation too severe: {}x size increase caused {}x time increase",
                size_ratio,
                time_ratio
            );
        }
    }
}
