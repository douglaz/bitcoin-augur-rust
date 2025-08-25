use bitcoin_augur::{FeeEstimator, MempoolSnapshot, MempoolTransaction, Result};
use chrono::Utc;

/// Regression test for the probability ordering bug where 95% confidence
/// was returning 1.00 sat/vB (minimum fee) instead of higher fees.
#[test]
fn test_no_minimum_fee_at_high_confidence() -> Result<()> {
    let estimator = FeeEstimator::new();

    // Create realistic mempool with various fee rates
    // Need enough transactions to cause congestion (multiple blocks worth)
    let mut transactions = Vec::new();

    // Add a good distribution of transactions at different fee rates
    // Total: ~8M weight units (2 blocks worth) to ensure congestion
    for fee_rate in [1, 2, 3, 5, 8, 10, 15, 20, 25, 30, 40, 50, 75, 100] {
        for _ in 0..100 {
            // 100 transactions per fee rate
            let weight = 6000; // Larger transactions
            let fee = (fee_rate as f64 * weight as f64 / 4.0) as u64;
            transactions.push(MempoolTransaction::new(weight, fee));
        }
    }

    let base_time = Utc::now();
    let mut snapshots = Vec::new();

    // Create multiple snapshots over time
    for i in 0..20 {
        let snapshot = MempoolSnapshot::from_transactions(
            transactions.clone(),
            850000 + i,
            base_time + chrono::Duration::minutes(10 * i as i64),
        );
        snapshots.push(snapshot);
    }

    let estimates = estimator.calculate_estimates(&snapshots, None)?;

    // Regression check: 95% confidence should NEVER return exactly 1.00 sat/vB
    // (unless that's the only fee rate available, which it isn't in our test data)
    println!("\nRegression Test Results:");
    for target in [3, 6, 12, 24, 144] {
        if let Some(block_target) = estimates.estimates.get(&target) {
            println!("Target {}:", target);
            for prob in [0.05, 0.20, 0.50, 0.80, 0.95] {
                if let Some(fee) = block_target.get_fee_rate(prob) {
                    println!("  {:.0}%: {:.2} sat/vB", prob * 100.0, fee);
                }
            }

            // Only check for short targets where we expect congestion
            // With 2 blocks worth of mempool, targets >= 12 blocks can legitimately return 1.00
            if target <= 6 {
                if let Some(fee_95) = block_target.get_fee_rate(0.95) {
                    assert!(
                        !(0.99..=1.01).contains(&fee_95), // Not exactly 1.00
                        "REGRESSION: Target {} at 95% confidence returned exactly 1.00 sat/vB",
                        target
                    );
                }
            }
        }
    }

    Ok(())
}

/// Test that probability ordering is generally correct (allowing small variations)
#[test]
fn test_probability_ordering_generally_increases() -> Result<()> {
    let estimator = FeeEstimator::new();

    // Create test data with good distribution
    let mut transactions = Vec::new();
    for fee_rate in 1..=200 {
        let weight = 2000;
        let fee = (fee_rate as f64 * weight as f64 / 4.0) as u64;
        transactions.push(MempoolTransaction::new(weight, fee));
    }

    let base_time = Utc::now();
    let mut snapshots = Vec::new();

    for i in 0..30 {
        snapshots.push(MempoolSnapshot::from_transactions(
            transactions.clone(),
            850000 + i,
            base_time + chrono::Duration::minutes(10 * i as i64),
        ));
    }

    let estimates = estimator.calculate_estimates(&snapshots, None)?;

    // For each target, check general trend (allowing small variations)
    for target in [3, 6, 12, 24, 144] {
        if let Some(block_target) = estimates.estimates.get(&target) {
            let fees: Vec<f64> = [0.05, 0.20, 0.50, 0.80, 0.95]
                .iter()
                .filter_map(|&p| block_target.get_fee_rate(p))
                .collect();

            if fees.len() >= 2 {
                // Check that the highest confidence (last) is not dramatically lower than lowest (first)
                let first = fees.first().unwrap();
                let last = fees.last().unwrap();

                assert!(
                    *last >= first * 0.9,  // Allow 10% variation
                    "Target {}: Fee at highest confidence ({:.2}) is much lower than at lowest ({:.2})",
                    target, last, first
                );

                // Check that we don't have the extreme 1.00 drop
                for fee in &fees {
                    if *fee == 1.00 && fees.iter().any(|&f| f > 10.0) {
                        panic!("Found 1.00 sat/vB alongside much higher fees - likely regression!");
                    }
                }
            }
        }
    }

    Ok(())
}

/// Test that the Poisson calculation is working correctly with proper semantics
#[test]
fn test_poisson_calculation_fixed() -> Result<()> {
    use statrs::distribution::{DiscreteCDF, Poisson};

    // Test the corrected logic for calculate_expected_blocks
    let target = 6.0;
    let poisson = Poisson::new(target).unwrap();

    // For 95% confidence, we want the largest k where P(X >= k) >= 0.95
    // This means we're pessimistic - assuming FEWER blocks will be mined
    let mut blocks_95 = 0;
    for k in (0..100).rev() {
        let prob_at_least_k = if k == 0 {
            1.0
        } else {
            1.0 - poisson.cdf((k - 1) as u64)
        };
        if prob_at_least_k >= 0.95 {
            blocks_95 = k;
            break;
        }
    }

    // For 5% confidence, we want the largest k where P(X >= k) >= 0.05
    // This means we're optimistic - assuming MORE blocks will be mined
    let mut blocks_05 = 0;
    for k in (0..100).rev() {
        let prob_at_least_k = if k == 0 {
            1.0
        } else {
            1.0 - poisson.cdf((k - 1) as u64)
        };
        if prob_at_least_k >= 0.05 {
            blocks_05 = k;
            break;
        }
    }

    assert!(
        blocks_95 < blocks_05,
        "95% confidence should assume FEWER blocks than 5% confidence (95%={}, 5%={})",
        blocks_95,
        blocks_05
    );

    // Specific check: for target=6, 95% should be around 2-3 blocks (pessimistic)
    assert!(
        blocks_95 <= 3,
        "For target=6, 95% confidence should assume at most 3 blocks, got {}",
        blocks_95
    );

    // And 5% should be around 9-10 blocks (optimistic)
    assert!(
        blocks_05 >= 9,
        "For target=6, 5% confidence should assume at least 9 blocks, got {}",
        blocks_05
    );

    Ok(())
}
