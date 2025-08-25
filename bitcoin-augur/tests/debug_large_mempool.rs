use bitcoin_augur::{FeeEstimator, MempoolSnapshot, MempoolTransaction, Result};
use chrono::Utc;

/// Test with large mempool that exceeds block capacity
#[test]
fn test_debug_large_mempool() -> Result<()> {
    let estimator = FeeEstimator::new();

    // Create large mempool that exceeds multiple blocks
    let mut transactions = Vec::new();

    // Add many transactions at different fee rates
    // Total: ~40M weight units (10 blocks worth)
    for fee_rate in [1, 2, 3, 5, 8, 10, 15, 20, 30, 50] {
        // 100 transactions * 4000 weight = 400,000 weight units per fee rate
        for _ in 0..100 {
            let weight = 4000; // Larger transaction
            let fee = (fee_rate as f64 * weight as f64 / 4.0) as u64;
            transactions.push(MempoolTransaction::new(weight, fee));
        }
    }

    let base_time = Utc::now();
    let mut snapshots = Vec::new();

    // Create snapshots
    for i in 0..10 {
        let snapshot = MempoolSnapshot::from_transactions(
            transactions.clone(),
            850000 + i,
            base_time + chrono::Duration::minutes(10 * i as i64),
        );
        snapshots.push(snapshot);
    }

    let total_weight: u64 = transactions.iter().map(|tx| tx.weight).sum();
    let blocks_needed = total_weight as f64 / 4_000_000.0;

    println!("\n=== Testing Large Mempool ===");
    println!("Transactions: {} total", transactions.len());
    println!(
        "Total weight: {} WU ({:.1} blocks worth)",
        total_weight, blocks_needed
    );
    println!("Snapshots: {} total", snapshots.len());

    let estimates = estimator.calculate_estimates(&snapshots, None)?;

    // Check estimates - should NOT all be 1.00
    println!("\nEstimates:");
    let mut all_ones = true;
    for target in [3, 6, 12, 24] {
        if let Some(block_target) = estimates.estimates.get(&target) {
            println!("Target {}:", target);
            for prob in [0.05, 0.50, 0.95] {
                if let Some(fee) = block_target.get_fee_rate(prob) {
                    println!("  {:.0}%: {:.2} sat/vB", prob * 100.0, fee);
                    if (fee - 1.00).abs() > 0.01 {
                        all_ones = false;
                    }
                }
            }
        }
    }

    if all_ones {
        println!("\nPROBLEM: All estimates are 1.00 despite large mempool!");
    } else {
        println!("\nGOOD: Got varied fee estimates for congested mempool");
    }

    Ok(())
}

/// Test with VERY congested mempool
#[test]
fn test_debug_very_congested_mempool() -> Result<()> {
    let estimator = FeeEstimator::new();

    // Create VERY large mempool - 100+ blocks worth
    let mut transactions = Vec::new();

    // Add massive number of transactions
    for fee_rate in 1..=100 {
        // More transactions at lower fee rates to simulate realistic distribution
        let count = if fee_rate < 10 {
            200
        } else if fee_rate < 30 {
            100
        } else {
            50
        };

        for _ in 0..count {
            let weight = 2000;
            let fee = (fee_rate as f64 * weight as f64 / 4.0) as u64;
            transactions.push(MempoolTransaction::new(weight, fee));
        }
    }

    let base_time = Utc::now();
    let mut snapshots = Vec::new();

    // Create multiple snapshots with growing mempool
    for i in 0..20 {
        // Add more transactions over time (inflow)
        if i > 0 && i % 5 == 0 {
            for fee_rate in [5, 10, 20, 40] {
                for _ in 0..10 {
                    let weight = 2000;
                    let fee = (fee_rate as f64 * weight as f64 / 4.0) as u64;
                    transactions.push(MempoolTransaction::new(weight, fee));
                }
            }
        }

        let snapshot = MempoolSnapshot::from_transactions(
            transactions.clone(),
            850000 + i,
            base_time + chrono::Duration::minutes(10 * i as i64),
        );
        snapshots.push(snapshot);
    }

    let total_weight: u64 = transactions.iter().map(|tx| tx.weight).sum();
    let blocks_needed = total_weight as f64 / 4_000_000.0;

    println!("\n=== Testing VERY Congested Mempool ===");
    println!("Transactions: {} total", transactions.len());
    println!(
        "Total weight: {} WU ({:.1} blocks worth!)",
        total_weight, blocks_needed
    );
    println!("Snapshots: {} total", snapshots.len());

    let estimates = estimator.calculate_estimates(&snapshots, None)?;

    // Check estimates - definitely should NOT all be 1.00
    println!("\nEstimates:");
    for target in [3, 6, 12, 24, 144] {
        if let Some(block_target) = estimates.estimates.get(&target) {
            println!("Target {}:", target);
            let mut last_fee = 0.0;
            for prob in [0.05, 0.20, 0.50, 0.80, 0.95] {
                if let Some(fee) = block_target.get_fee_rate(prob) {
                    println!("  {:.0}%: {:.2} sat/vB", prob * 100.0, fee);

                    // Check monotonicity
                    if fee < last_fee && last_fee > 0.0 {
                        println!("    WARNING: Fee decreased!");
                    }
                    last_fee = fee;
                }
            }
        }
    }

    // Check that 95% confidence for short targets is > 1.00
    if let Some(block_target) = estimates.estimates.get(&3) {
        if let Some(fee_95) = block_target.get_fee_rate(0.95) {
            if fee_95 <= 1.01 {
                println!("\nPROBLEM: 3-block target at 95% confidence is still ~1.00 sat/vB!");
                println!(
                    "This suggests the algorithm isn't working correctly for congested mempools."
                );
            } else {
                println!(
                    "\nGOOD: 3-block target at 95% confidence is {:.2} sat/vB",
                    fee_95
                );
            }
        }
    }

    Ok(())
}
