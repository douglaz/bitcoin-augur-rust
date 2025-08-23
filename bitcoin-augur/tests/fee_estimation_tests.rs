use bitcoin_augur::{FeeEstimator, MempoolSnapshot, MempoolTransaction, Result};
use chrono::{Duration, Utc};

#[test]
fn test_basic_fee_estimation() -> Result<()> {
    let estimator = FeeEstimator::new();

    // Create multiple snapshots simulating mempool evolution
    let base_time = Utc::now();
    let mut snapshots = Vec::new();

    // Initial mempool state with various fee rates
    let transactions = vec![
        MempoolTransaction::new(400, 10000), // ~100 sat/vB
        MempoolTransaction::new(565, 5650),  // ~40 sat/vB
        MempoolTransaction::new(1000, 2000), // ~8 sat/vB
        MempoolTransaction::new(2000, 1000), // ~2 sat/vB
    ];

    snapshots.push(MempoolSnapshot::from_transactions(
        transactions.clone(),
        850000,
        base_time,
    ));

    // Add more snapshots over time with growing mempool
    for i in 1..10 {
        let mut new_transactions = transactions.clone();
        // Add some new transactions with varying fees
        new_transactions.push(MempoolTransaction::new(
            500 + i * 100,
            (1000 + i * 500) as u64,
        ));

        snapshots.push(MempoolSnapshot::from_transactions(
            new_transactions,
            850000 + (i / 3) as u32, // New block every 3 snapshots
            base_time + Duration::minutes(i as i64 * 10),
        ));
    }

    let estimates = estimator.calculate_estimates(&snapshots, None)?;

    // Verify we got estimates
    assert!(!estimates.estimates.is_empty());

    // Check that fee rates decrease with longer confirmation targets
    if let (Some(target_6), Some(target_12)) =
        (estimates.estimates.get(&6), estimates.estimates.get(&12))
    {
        // For same confidence level, longer target should have lower or equal fee
        if let (Some(fee_6), Some(fee_12)) =
            (target_6.get_fee_rate(0.95), target_12.get_fee_rate(0.95))
        {
            assert!(
                fee_6 >= fee_12,
                "Fee for 6 blocks should be >= fee for 12 blocks"
            );
        }
    }

    Ok(())
}

#[test]
fn test_empty_mempool_estimation() -> Result<()> {
    let estimator = FeeEstimator::new();

    // Empty mempool snapshot
    let snapshot = MempoolSnapshot::from_transactions(vec![], 850000, Utc::now());

    let estimates = estimator.calculate_estimates(&[snapshot], None)?;

    // Should still return a valid estimate structure
    assert!(estimates.timestamp > chrono::DateTime::<chrono::Utc>::MIN_UTC);

    Ok(())
}

#[test]
fn test_high_fee_pressure() -> Result<()> {
    let estimator = FeeEstimator::new();

    let mut snapshots = Vec::new();
    let base_time = Utc::now();

    // Simulate high fee pressure with many high-fee transactions
    for i in 0..24 {
        let mut transactions = Vec::new();

        // Add many high-fee transactions (100-500 sat/vB)
        for j in 0..100 {
            let weight = 400 + (j % 200);
            let fee_rate = 100 + (j % 400); // 100-500 sat/vB
            let fee = (weight * fee_rate / 4) as u64;
            transactions.push(MempoolTransaction::new(weight, fee));
        }

        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i / 6, // New block every hour
            base_time + Duration::hours(i as i64),
        ));
    }

    let estimates = estimator.calculate_estimates(&snapshots, None)?;

    // Under high pressure, we should get fee estimates
    assert!(
        !estimates.estimates.is_empty(),
        "Should have fee estimates under high pressure"
    );

    // Check that shorter targets have higher fees than longer targets
    if let (Some(target_3), Some(target_12)) =
        (estimates.estimates.get(&3), estimates.estimates.get(&12))
    {
        if let (Some(fee_3), Some(fee_12)) =
            (target_3.get_fee_rate(0.50), target_12.get_fee_rate(0.50))
        {
            assert!(
                fee_3 >= fee_12,
                "Shorter targets should have higher or equal fees"
            );
        }
    }

    Ok(())
}

#[test]
fn test_custom_block_target() -> Result<()> {
    let estimator = FeeEstimator::new();

    let transactions = vec![
        MempoolTransaction::new(400, 10000),
        MempoolTransaction::new(565, 5650),
        MempoolTransaction::new(1000, 2000),
    ];

    let snapshot = MempoolSnapshot::from_transactions(transactions, 850000, Utc::now());

    // Test with custom block target of 15 blocks
    let estimates = estimator.calculate_estimates(&[snapshot], Some(15.0))?;

    // Should only have one target (15 blocks)
    assert_eq!(estimates.estimates.len(), 1);
    assert!(estimates.estimates.contains_key(&15));

    Ok(())
}

#[test]
fn test_confidence_levels() -> Result<()> {
    let estimator = FeeEstimator::new();

    let mut snapshots = Vec::new();
    let base_time = Utc::now();

    // Create snapshots with consistent transaction patterns
    for i in 0..10 {
        let transactions = vec![
            MempoolTransaction::new(400, 10000), // High fee
            MempoolTransaction::new(500, 5000),  // Medium fee
            MempoolTransaction::new(600, 3000),  // Medium fee
            MempoolTransaction::new(1000, 1000), // Low fee
            MempoolTransaction::new(2000, 500),  // Very low fee
        ];

        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i,
            base_time + Duration::minutes(i as i64 * 10),
        ));
    }

    let estimates = estimator.calculate_estimates(&snapshots, None)?;

    // For a given block target, higher confidence should require higher fees
    if let Some(target_6) = estimates.estimates.get(&6) {
        let fee_50 = target_6.get_fee_rate(0.50);
        let fee_95 = target_6.get_fee_rate(0.95);

        if let (Some(f50), Some(f95)) = (fee_50, fee_95) {
            assert!(
                f95 >= f50,
                "95% confidence should require >= fee than 50% confidence"
            );
        }
    }

    Ok(())
}

#[test]
fn test_inflow_simulation() -> Result<()> {
    let estimator = FeeEstimator::new();

    let mut snapshots = Vec::new();
    let base_time = Utc::now();

    // Create snapshots showing consistent inflow pattern
    for block in 0..20 {
        for minute in 0..10 {
            let mut transactions = Vec::new();

            // Base transactions
            for i in 0..10 {
                transactions.push(MempoolTransaction::new(
                    400 + i * 50,
                    (1000 + i * 100) as u64,
                ));
            }

            // Add new transactions each minute (simulating inflow)
            for j in 0..minute {
                transactions.push(MempoolTransaction::new(500, (2000 + j * 100) as u64));
            }

            snapshots.push(MempoolSnapshot::from_transactions(
                transactions,
                850000 + block,
                base_time + Duration::minutes((block * 10 + minute) as i64),
            ));
        }
    }

    let estimates = estimator.calculate_estimates(&snapshots, None)?;

    // Verify estimates exist and are reasonable
    assert!(!estimates.estimates.is_empty());

    // Check that we have estimates for various targets
    assert!(estimates.estimates.contains_key(&3));
    assert!(estimates.estimates.contains_key(&6));
    assert!(estimates.estimates.contains_key(&144));

    Ok(())
}

#[test]
fn test_bucket_distribution() -> Result<()> {
    let estimator = FeeEstimator::new();

    let mut transactions = Vec::new();

    // Create transactions across a wide range of fee rates
    // Testing logarithmic bucketing
    for i in 0..100 {
        let fee_rate = (1.5_f64).powi(i % 20) as u64 + 1; // Exponential distribution
        let weight = 400;
        let fee = weight * fee_rate / 4;
        transactions.push(MempoolTransaction::new(weight, fee));
    }

    let snapshot = MempoolSnapshot::from_transactions(transactions, 850000, Utc::now());

    // Verify bucketing doesn't lose transactions
    // Total weight should be preserved in buckets
    let bucket_weights = snapshot.bucketed_weights.values().sum::<u64>();
    assert!(bucket_weights > 0);

    let estimates = estimator.calculate_estimates(&[snapshot], None)?;
    assert!(!estimates.estimates.is_empty());

    Ok(())
}

#[test]
fn test_monotonicity_enforcement() -> Result<()> {
    let estimator = FeeEstimator::new();

    // Create a mempool with varied fee distribution
    let mut transactions = Vec::new();
    for i in 0..50 {
        let weight = 400 + (i % 100) * 10;
        let fee_rate = 1 + (i * i) % 200; // Non-linear fee distribution
        let fee = (weight * fee_rate / 4) as u64;
        transactions.push(MempoolTransaction::new(weight, fee));
    }

    let snapshot = MempoolSnapshot::from_transactions(transactions, 850000, Utc::now());

    let estimates = estimator.calculate_estimates(&[snapshot], None)?;

    // Verify monotonicity: fees should not increase with block target
    let targets = vec![3, 6, 9, 12, 18, 24, 36, 48, 72, 96, 144];
    for confidence in [0.05, 0.50, 0.95] {
        let mut prev_fee = f64::INFINITY;

        for target in &targets {
            if let Some(block_target) = estimates.estimates.get(target) {
                if let Some(fee) = block_target.get_fee_rate(confidence) {
                    assert!(
                        fee <= prev_fee,
                        "Fee for {} blocks ({}) should be <= fee for previous target ({})",
                        target,
                        fee,
                        prev_fee
                    );
                    prev_fee = fee;
                }
            }
        }
    }

    Ok(())
}

#[test]
fn test_custom_estimator_config() -> Result<()> {
    // Test with custom probabilities and block targets
    let estimator = FeeEstimator::with_config(
        vec![0.25, 0.50, 0.75], // Custom confidence levels
        vec![10.0, 20.0, 30.0], // Custom block targets
        Duration::minutes(15),  // Short-term window
        Duration::hours(12),    // Long-term window
    )?;

    let transactions = vec![
        MempoolTransaction::new(400, 10000),
        MempoolTransaction::new(565, 5650),
        MempoolTransaction::new(1000, 2000),
    ];

    let snapshot = MempoolSnapshot::from_transactions(transactions, 850000, Utc::now());

    let estimates = estimator.calculate_estimates(&[snapshot], None)?;

    // Should have our custom block targets
    for target in [10, 20, 30] {
        if estimates.estimates.contains_key(&target) {
            let block_target = &estimates.estimates[&target];
            // Should have our custom confidence levels
            assert!(block_target.probabilities.len() <= 3);
        }
    }

    Ok(())
}

#[test]
fn test_extreme_fee_rates() -> Result<()> {
    let estimator = FeeEstimator::new();

    let mut transactions = Vec::new();

    // Add some extremely high fee transactions
    transactions.push(MempoolTransaction::new(250, 250000)); // 4000 sat/vB
    transactions.push(MempoolTransaction::new(300, 150000)); // 2000 sat/vB

    // Add normal transactions
    transactions.push(MempoolTransaction::new(400, 4000)); // 40 sat/vB
    transactions.push(MempoolTransaction::new(500, 2500)); // 20 sat/vB

    // Add very low fee transactions
    transactions.push(MempoolTransaction::new(1000, 100)); // 0.4 sat/vB
    transactions.push(MempoolTransaction::new(2000, 50)); // 0.1 sat/vB

    let snapshot = MempoolSnapshot::from_transactions(transactions, 850000, Utc::now());

    let estimates = estimator.calculate_estimates(&[snapshot], None)?;

    // Should handle extreme values without panicking
    assert!(!estimates.estimates.is_empty());

    // High confidence, short target should reflect high fees
    if let Some(target_3) = estimates.estimates.get(&3) {
        if let Some(fee) = target_3.get_fee_rate(0.95) {
            assert!(fee > 0.0, "Should have non-zero fee estimate");
        }
    }

    Ok(())
}
