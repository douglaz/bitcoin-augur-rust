use bitcoin_augur::{FeeEstimator, MempoolSnapshot, MempoolTransaction, Result};
use chrono::Utc;

/// Debug test for why simple data returns all 1.00 sat/vB
#[test]
fn test_debug_simple_data() -> Result<()> {
    let estimator = FeeEstimator::new();

    // Create VERY simple mempool - just a few transactions
    let transactions = vec![
        MempoolTransaction::new(1000, 250),  // 1.00 sat/vB
        MempoolTransaction::new(1000, 500),  // 2.00 sat/vB
        MempoolTransaction::new(1000, 1000), // 4.00 sat/vB
        MempoolTransaction::new(1000, 2000), // 8.00 sat/vB
    ];

    let base_time = Utc::now();
    let mut snapshots = Vec::new();

    // Create just 2 snapshots (minimum required)
    for i in 0..2 {
        let snapshot = MempoolSnapshot::from_transactions(
            transactions.clone(),
            850000 + i,
            base_time + chrono::Duration::minutes(10 * i as i64),
        );
        snapshots.push(snapshot);
    }

    println!("\n=== Testing Simple Data ===");
    println!("Transactions: {} total", transactions.len());
    println!("Snapshots: {} total", snapshots.len());

    let estimates = estimator.calculate_estimates(&snapshots, None)?;

    // Check what we get
    println!("\nEstimates:");
    for target in [3, 6, 12] {
        if let Some(block_target) = estimates.estimates.get(&target) {
            println!("Target {}:", target);
            for prob in [0.05, 0.50, 0.95] {
                if let Some(fee) = block_target.get_fee_rate(prob) {
                    println!("  {:.0}%: {:.2} sat/vB", prob * 100.0, fee);
                }
            }
        }
    }

    Ok(())
}

/// Test with slightly more complex data
#[test]
fn test_debug_moderate_data() -> Result<()> {
    let estimator = FeeEstimator::new();

    // Create more realistic mempool
    let mut transactions = Vec::new();
    for fee_rate in [1, 2, 3, 5, 8, 10, 15, 20] {
        for _ in 0..5 {
            let weight = 1000;
            let fee = (fee_rate as f64 * weight as f64 / 4.0) as u64;
            transactions.push(MempoolTransaction::new(weight, fee));
        }
    }

    let base_time = Utc::now();
    let mut snapshots = Vec::new();

    // Create 10 snapshots
    for i in 0..10 {
        let snapshot = MempoolSnapshot::from_transactions(
            transactions.clone(),
            850000 + i,
            base_time + chrono::Duration::minutes(10 * i as i64),
        );
        snapshots.push(snapshot);
    }

    println!("\n=== Testing Moderate Data ===");
    println!("Transactions: {} total", transactions.len());
    println!("Snapshots: {} total", snapshots.len());

    let estimates = estimator.calculate_estimates(&snapshots, None)?;

    // Check what we get
    println!("\nEstimates:");
    for target in [3, 6, 12] {
        if let Some(block_target) = estimates.estimates.get(&target) {
            println!("Target {}:", target);
            for prob in [0.05, 0.50, 0.95] {
                if let Some(fee) = block_target.get_fee_rate(prob) {
                    println!("  {:.0}%: {:.2} sat/vB", prob * 100.0, fee);

                    // Check if all are 1.00
                    if (fee - 1.00).abs() < 0.01 {
                        println!("    WARNING: Got 1.00 sat/vB!");
                    }
                }
            }
        }
    }

    Ok(())
}

/// Test with no mempool activity (static mempool)
#[test]
fn test_debug_static_mempool() -> Result<()> {
    let estimator = FeeEstimator::new();

    // Create static mempool (same transactions in all snapshots)
    let transactions = vec![
        MempoolTransaction::new(1000, 250),  // 1.00 sat/vB
        MempoolTransaction::new(1000, 500),  // 2.00 sat/vB
        MempoolTransaction::new(1000, 1000), // 4.00 sat/vB
        MempoolTransaction::new(1000, 2000), // 8.00 sat/vB
        MempoolTransaction::new(1000, 4000), // 16.00 sat/vB
    ];

    let base_time = Utc::now();
    let mut snapshots = Vec::new();

    // Create multiple snapshots with SAME content
    for i in 0..10 {
        let snapshot = MempoolSnapshot::from_transactions(
            transactions.clone(),
            850000 + i,
            base_time + chrono::Duration::minutes(10 * i as i64),
        );
        snapshots.push(snapshot);
    }

    println!("\n=== Testing Static Mempool ===");
    println!("Transactions: {} (unchanging)", transactions.len());
    println!("Snapshots: {} total", snapshots.len());

    let estimates = estimator.calculate_estimates(&snapshots, None)?;

    // Check what we get
    println!("\nEstimates:");
    for target in [3, 6, 12] {
        if let Some(block_target) = estimates.estimates.get(&target) {
            println!("Target {}:", target);
            for prob in [0.05, 0.50, 0.95] {
                if let Some(fee) = block_target.get_fee_rate(prob) {
                    println!("  {:.0}%: {:.2} sat/vB", prob * 100.0, fee);

                    // Static mempool might legitimately return 1.00
                    // if there's no congestion
                    if (fee - 1.00).abs() < 0.01 {
                        println!("    INFO: 1.00 sat/vB (expected for static mempool)");
                    }
                }
            }
        }
    }

    Ok(())
}
