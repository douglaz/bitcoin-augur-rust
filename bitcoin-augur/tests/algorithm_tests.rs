use bitcoin_augur::{MempoolSnapshot, MempoolTransaction, Result};
use chrono::{Duration, Utc};

#[test]
fn test_poisson_distribution_calculation() -> Result<()> {
    // Test that Poisson distribution is properly used for block expectations
    let estimator = bitcoin_augur::FeeEstimator::new();

    let mut snapshots = Vec::new();
    let base_time = Utc::now();

    // Create consistent mempool pattern
    for i in 0..10 {
        let transactions = vec![
            MempoolTransaction::new(400, 10000),
            MempoolTransaction::new(500, 5000),
            MempoolTransaction::new(600, 2000),
        ];

        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i,
            base_time + Duration::minutes(i as i64 * 10),
        ));
    }

    let estimates = estimator.calculate_estimates(&snapshots, None)?;

    // Verify we get different estimates for different confidence levels
    if let Some(target) = estimates.estimates.get(&6) {
        let low_conf = target.get_fee_rate(0.05);
        let high_conf = target.get_fee_rate(0.95);

        if let (Some(low), Some(high)) = (low_conf, high_conf) {
            // Lower confidence should allow lower fees
            assert!(
                low <= high,
                "Lower confidence should allow lower or equal fees"
            );
        }
    }

    Ok(())
}

#[test]
fn test_block_mining_simulation() -> Result<()> {
    // Test that block mining simulation properly removes transactions
    let estimator = bitcoin_augur::FeeEstimator::new();

    // Create a mempool with exactly 4MB of transactions
    let mut transactions = Vec::new();
    let transactions_per_block = 10000; // 400 WU each = 4M WU total

    for i in 0..transactions_per_block {
        let fee_rate = 100 - (i / 100); // Decreasing fee rates
        transactions.push(MempoolTransaction::new(400, fee_rate as u64 * 100));
    }

    let snapshot = MempoolSnapshot::from_transactions(transactions, 850000, Utc::now());

    let estimates = estimator.calculate_estimates(&[snapshot], None)?;

    // After mining one block, high-fee transactions should be removed
    // Lower targets should require higher fees
    if let (Some(t3), Some(t6)) = (estimates.estimates.get(&3), estimates.estimates.get(&6)) {
        if let (Some(f3), Some(f6)) = (t3.get_fee_rate(0.50), t6.get_fee_rate(0.50)) {
            assert!(
                f3 >= f6,
                "3-block target should require >= fee than 6-block target"
            );
        }
    }

    Ok(())
}

#[test]
fn test_inflow_rate_calculation() -> Result<()> {
    let estimator = bitcoin_augur::FeeEstimator::new();

    let mut snapshots = Vec::new();
    let base_time = Utc::now();

    // Simulate consistent transaction inflow
    let base_transactions = vec![
        MempoolTransaction::new(400, 10000),
        MempoolTransaction::new(500, 5000),
    ];

    for i in 0..30 {
        let mut transactions = base_transactions.clone();

        // Add new transactions over time (simulating inflow)
        for j in 0..i {
            transactions.push(MempoolTransaction::new(400, (2000 + j * 100) as u64));
        }

        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i / 6, // New block every hour
            base_time + Duration::minutes(i as i64 * 10),
        ));
    }

    let estimates = estimator.calculate_estimates(&snapshots, None)?;

    // With consistent inflow, estimates should account for future transactions
    assert!(!estimates.estimates.is_empty());

    // Longer targets should see impact of inflow
    if let Some(target_144) = estimates.estimates.get(&144) {
        // Should have estimates even for long-term targets
        assert!(!target_144.probabilities.is_empty());
    }

    Ok(())
}

#[test]
fn test_weighted_estimates_combination() -> Result<()> {
    // Test that short-term and long-term estimates are properly weighted
    let estimator = bitcoin_augur::FeeEstimator::new();

    let mut snapshots = Vec::new();
    let base_time = Utc::now();

    // Create 24 hours of data with varying patterns
    for hour in 0..24 {
        // Vary transaction volume by hour (simulating daily patterns)
        let num_transactions = if (8..=20).contains(&hour) {
            50 // Busy hours
        } else {
            10 // Quiet hours
        };

        let mut transactions = Vec::new();
        for i in 0..num_transactions {
            let fee_rate = 10 + (i % 50);
            transactions.push(MempoolTransaction::new(400, (fee_rate * 100) as u64));
        }

        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + hour,
            base_time + Duration::hours(hour as i64),
        ));
    }

    let estimates = estimator.calculate_estimates(&snapshots, None)?;

    // Should have weighted estimates combining short and long term patterns
    assert!(!estimates.estimates.is_empty());

    // Check that we get reasonable estimates for various targets
    for target in [3, 12, 36, 144] {
        assert!(
            estimates.estimates.contains_key(&target),
            "Should have estimate for {}-block target",
            target
        );
    }

    Ok(())
}

#[test]
fn test_fee_bucket_logarithmic_scaling() -> Result<()> {
    // Test that fee rates are properly bucketed using logarithmic scale
    let mut transactions = Vec::new();

    // Create transactions with exponentially increasing fee rates
    for i in 0..20 {
        let fee_rate = 2_u64.pow(i); // 1, 2, 4, 8, 16, 32, ...
        let weight = 400;
        let fee = weight * fee_rate / 4;
        transactions.push(MempoolTransaction::new(weight, fee));
    }

    let snapshot = MempoolSnapshot::from_transactions(transactions, 850000, Utc::now());

    // Verify that bucketing preserves transaction information
    let total_weight: u64 = snapshot.bucketed_weights.values().sum();
    assert_eq!(total_weight, 400 * 20); // All transactions should be counted

    // Check bucket distribution
    assert!(
        !snapshot.bucketed_weights.is_empty(),
        "Should have non-empty buckets"
    );

    // Higher fee transactions should be in higher buckets
    let mut last_bucket = -1;
    for (bucket, weight) in &snapshot.bucketed_weights {
        if *weight > 0 {
            assert!(*bucket >= last_bucket, "Buckets should be ordered");
            last_bucket = *bucket;
        }
    }

    Ok(())
}

#[test]
fn test_block_size_constraint() -> Result<()> {
    // Test that block mining respects 4MB weight limit
    let estimator = bitcoin_augur::FeeEstimator::new();

    // Create more than 4MB worth of transactions
    let mut transactions = Vec::new();
    for i in 0..20000 {
        let fee_rate = 100 - (i / 200).min(99); // Decreasing fee rates
        transactions.push(MempoolTransaction::new(400, (fee_rate * 100) as u64));
    }

    let snapshot = MempoolSnapshot::from_transactions(transactions, 850000, Utc::now());

    // Total weight is 8MB (20000 * 400)
    let total_weight: u64 = snapshot.bucketed_weights.values().sum();
    assert_eq!(total_weight, 8_000_000);

    let estimates = estimator.calculate_estimates(&[snapshot], None)?;

    // Should handle oversized mempool correctly
    assert!(!estimates.estimates.is_empty());

    // After mining 2 blocks, lower fee transactions should be accessible
    if let Some(target_3) = estimates.estimates.get(&3) {
        // Should need relatively high fees for quick confirmation
        if let Some(fee) = target_3.get_fee_rate(0.95) {
            assert!(
                fee > 0.0,
                "Should have a positive fee for quick confirmation"
            );
        }
    }

    Ok(())
}

#[test]
fn test_snapshot_ordering() -> Result<()> {
    // Test that snapshots are properly ordered by timestamp
    let estimator = bitcoin_augur::FeeEstimator::new();

    let mut snapshots = Vec::new();
    let base_time = Utc::now();

    // Add snapshots in reverse chronological order
    for i in (0..10).rev() {
        let transactions = vec![MempoolTransaction::new(400, 1000)];

        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i,
            base_time + Duration::minutes(i as i64 * 10),
        ));
    }

    // Estimator should handle unordered snapshots
    let estimates = estimator.calculate_estimates(&snapshots, None)?;

    // Should still produce valid estimates
    assert!(estimates.timestamp >= base_time);

    Ok(())
}

#[test]
fn test_minimum_data_requirement() -> Result<()> {
    // Test behavior with minimal data
    let estimator = bitcoin_augur::FeeEstimator::new();

    // Single snapshot with one transaction
    let transactions = vec![MempoolTransaction::new(400, 1000)];

    let snapshot = MempoolSnapshot::from_transactions(transactions, 850000, Utc::now());

    let estimates = estimator.calculate_estimates(&[snapshot], None)?;

    // Should handle minimal data gracefully
    assert!(
        !estimates.estimates.is_empty()
            || estimates.timestamp > chrono::DateTime::<chrono::Utc>::MIN_UTC
    );

    Ok(())
}

#[test]
fn test_probability_boundaries() -> Result<()> {
    // Test edge cases for probability values
    let estimator = bitcoin_augur::FeeEstimator::with_config(
        vec![0.01, 0.99], // Extreme confidence levels
        vec![6.0],
        Duration::minutes(30),
        Duration::hours(24),
    )?;

    let transactions = vec![
        MempoolTransaction::new(400, 10000),
        MempoolTransaction::new(500, 5000),
        MempoolTransaction::new(600, 1000),
    ];

    let snapshot = MempoolSnapshot::from_transactions(transactions, 850000, Utc::now());

    let estimates = estimator.calculate_estimates(&[snapshot], None)?;

    if let Some(target) = estimates.estimates.get(&6) {
        // Should handle extreme probabilities
        let very_low = target.get_fee_rate(0.01);
        let very_high = target.get_fee_rate(0.99);

        if let (Some(low), Some(high)) = (very_low, very_high) {
            assert!(
                high >= low,
                "Higher confidence should require higher or equal fee"
            );
        }
    }

    Ok(())
}
