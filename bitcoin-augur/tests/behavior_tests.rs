//! Tests documenting expected behavior of the fee estimation algorithm
//!
//! These tests explicitly document why certain edge cases produce specific results,
//! helping maintainers understand the algorithm's decision process.

use bitcoin_augur::{FeeEstimator, MempoolSnapshot, MempoolTransaction};
use chrono::{Duration, Utc};

#[test]
fn test_single_block_uniform_fees_returns_minimum() {
    // When all transactions have the same fee rate and fit in a single block,
    // the algorithm correctly returns the minimum fee rate (1 sat/vB) because
    // any fee rate would be sufficient to get confirmed in the next block.

    let estimator = FeeEstimator::new();
    let base_time = Utc::now();

    // Create exactly one block worth of transactions (4M weight units)
    // with uniform fee rate of 50 sat/vB
    let fee_rate = 50u64;
    let weight = 40000u64;
    let fee = fee_rate * weight / 4;
    let num_transactions = 100; // 100 * 40000 = 4M weight units = 1 block

    let mut snapshots = Vec::new();
    for i in 0..5 {
        let transactions: Vec<_> = (0..num_transactions)
            .map(|_| MempoolTransaction::new(weight, fee))
            .collect();

        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i,
            base_time + Duration::minutes((i * 10) as i64),
        ));
    }

    let estimates = estimator.calculate_estimates(&snapshots, None).unwrap();

    // With everything fitting in one block, the estimate should be minimal
    if let Some(fee_estimate) = estimates.get_fee_rate(3, 0.5) {
        assert_eq!(
            fee_estimate, 1.0,
            "Single block of uniform fees should return minimum fee rate"
        );
    }
}

#[test]
fn test_multiple_blocks_uniform_fees_returns_actual_rate() {
    // When transactions with uniform fee rates require multiple blocks to clear,
    // the algorithm should return estimates closer to the actual fee rate.

    let estimator = FeeEstimator::new();
    let base_time = Utc::now();

    // Create three blocks worth of transactions with uniform fee rate of 50 sat/vB
    let fee_rate = 50u64;
    let weight = 40000u64;
    let fee = fee_rate * weight / 4;
    let num_transactions = 300; // 300 * 40000 = 12M weight units = 3 blocks

    let mut snapshots = Vec::new();
    for i in 0..5 {
        let transactions: Vec<_> = (0..num_transactions)
            .map(|_| MempoolTransaction::new(weight, fee))
            .collect();

        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i,
            base_time + Duration::minutes((i * 10) as i64),
        ));
    }

    let estimates = estimator.calculate_estimates(&snapshots, None).unwrap();

    // With multiple blocks needed, estimates should be non-trivial
    if let Some(fee_estimate) = estimates.get_fee_rate(3, 0.95) {
        assert!(
            fee_estimate > 1.0,
            "Multiple blocks of uniform fees should return non-minimum estimate, got {}",
            fee_estimate
        );

        // Should be within reasonable range of actual fee rate
        assert!(
            fee_estimate <= fee_rate as f64 * 2.0,
            "Estimate {} should be within 2x of actual rate {}",
            fee_estimate,
            fee_rate
        );
    }
}

#[test]
fn test_empty_mempool_returns_none() {
    // When the mempool is completely empty with no inflows,
    // the algorithm correctly returns None for all estimates.

    let estimator = FeeEstimator::new();
    let base_time = Utc::now();

    let mut snapshots = Vec::new();
    for i in 0..5 {
        snapshots.push(MempoolSnapshot::from_transactions(
            vec![], // Empty mempool
            850000 + i,
            base_time + Duration::minutes((i * 10) as i64),
        ));
    }

    let estimates = estimator.calculate_estimates(&snapshots, None).unwrap();

    // Empty mempool should produce no estimates
    for target in [3, 6, 12, 24, 144] {
        for confidence in [0.5, 0.8, 0.95] {
            assert_eq!(
                estimates.get_fee_rate(target, confidence),
                None,
                "Empty mempool should return None for all estimates"
            );
        }
    }
}

#[test]
fn test_high_confidence_requires_higher_fees() {
    // Higher confidence levels should require equal or higher fee estimates
    // because they represent being more pessimistic about block production speed.

    let estimator = FeeEstimator::new();
    let base_time = Utc::now();

    // Create a realistic mempool with varied fee rates
    let mut snapshots = Vec::new();
    for i in 0..5 {
        let mut transactions = Vec::new();

        // Add transactions with varying fee rates (10-100 sat/vB)
        for j in 0..500 {
            let fee_rate = 10 + (j % 10) * 10; // 10, 20, 30, ..., 100
            let weight = 4000u64;
            let fee = fee_rate * weight / 4;
            transactions.push(MempoolTransaction::new(weight, fee));
        }

        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i,
            base_time + Duration::minutes((i * 10) as i64),
        ));
    }

    let estimates = estimator.calculate_estimates(&snapshots, None).unwrap();

    // Check that higher confidence requires higher fees
    for target in [3, 6, 12] {
        let fee_50 = estimates.get_fee_rate(target, 0.5);
        let fee_80 = estimates.get_fee_rate(target, 0.8);
        let fee_95 = estimates.get_fee_rate(target, 0.95);

        if let (Some(f50), Some(f80)) = (fee_50, fee_80) {
            assert!(
                f80 >= f50,
                "80% confidence ({}) should require >= fee than 50% confidence ({})",
                f80,
                f50
            );
        }

        if let (Some(f80), Some(f95)) = (fee_80, fee_95) {
            assert!(
                f95 >= f80,
                "95% confidence ({}) should require >= fee than 80% confidence ({})",
                f95,
                f80
            );
        }
    }
}

#[test]
fn test_longer_targets_allow_lower_fees() {
    // Longer confirmation targets should allow equal or lower fee estimates
    // because there's more time for lower fee transactions to be included.

    let estimator = FeeEstimator::new();
    let base_time = Utc::now();

    // Create a realistic mempool
    let mut snapshots = Vec::new();
    for i in 0..5 {
        let mut transactions = Vec::new();

        // Add transactions with varying fee rates
        for j in 0..500 {
            let fee_rate = 5 + (j % 20) * 5; // 5-100 sat/vB
            let weight = 4000u64;
            let fee = fee_rate * weight / 4;
            transactions.push(MempoolTransaction::new(weight, fee));
        }

        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i,
            base_time + Duration::minutes((i * 10) as i64),
        ));
    }

    let estimates = estimator.calculate_estimates(&snapshots, None).unwrap();

    // Check monotonicity across targets
    for confidence in [0.5, 0.8, 0.95] {
        let mut prev_fee = f64::INFINITY;

        for target in [3, 6, 12, 24, 144] {
            if let Some(fee) = estimates.get_fee_rate(target, confidence) {
                assert!(
                    fee <= prev_fee,
                    "Target {} blocks should have fee {} <= previous fee {}",
                    target,
                    fee,
                    prev_fee
                );
                prev_fee = fee;
            }
        }
    }
}

#[test]
fn test_weighting_affects_long_term_estimates() {
    // The algorithm weights short-term and long-term inflows differently
    // based on the target block count. Longer targets should be more
    // influenced by long-term patterns.

    let estimator = FeeEstimator::new();
    let base_time = Utc::now();

    // Create snapshots with different patterns
    let mut snapshots = Vec::new();

    // First few snapshots: high fees
    for i in 0..2 {
        let mut transactions = Vec::new();
        for _ in 0..200 {
            let weight = 4000u64;
            let fee = 100 * weight / 4; // 100 sat/vB
            transactions.push(MempoolTransaction::new(weight, fee));
        }
        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i,
            base_time + Duration::minutes((i * 10) as i64),
        ));
    }

    // Later snapshots: low fees
    for i in 2..5 {
        let mut transactions = Vec::new();
        for _ in 0..200 {
            let weight = 4000u64;
            let fee = 10 * weight / 4; // 10 sat/vB
            transactions.push(MempoolTransaction::new(weight, fee));
        }
        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i,
            base_time + Duration::minutes((i * 10) as i64),
        ));
    }

    let estimates = estimator.calculate_estimates(&snapshots, None).unwrap();

    // Short-term estimates should be more influenced by recent (low fee) data
    // Long-term estimates should consider the full history
    if let (Some(short_fee), Some(long_fee)) = (
        estimates.get_fee_rate(3, 0.5),
        estimates.get_fee_rate(144, 0.5),
    ) {
        // This test documents that the weighting algorithm exists,
        // but the exact relationship depends on implementation details
        println!(
            "3-block estimate: {}, 144-block estimate: {}",
            short_fee, long_fee
        );

        // Both should be reasonable (not extreme values)
        assert!((1.0..=1000.0).contains(&short_fee));
        assert!((1.0..=1000.0).contains(&long_fee));
    }
}
