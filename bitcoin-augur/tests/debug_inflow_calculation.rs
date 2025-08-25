use bitcoin_augur::{FeeEstimator, MempoolSnapshot, MempoolTransaction, Result};
use chrono::Utc;

/// Debug test to understand inflow calculation
#[test]
fn test_debug_inflow_calculation() -> Result<()> {
    let estimator = FeeEstimator::new();

    // Create changing mempool to generate inflow
    let base_time = Utc::now();
    let mut snapshots = Vec::new();

    // First snapshot: small mempool
    let snap1_txs = vec![
        MempoolTransaction::new(1000, 250), // 1.00 sat/vB
        MempoolTransaction::new(1000, 500), // 2.00 sat/vB
    ];
    snapshots.push(MempoolSnapshot::from_transactions(
        snap1_txs, 850000, base_time,
    ));

    // Second snapshot: more transactions added (inflow)
    let snap2_txs = vec![
        MempoolTransaction::new(1000, 250),  // 1.00 sat/vB
        MempoolTransaction::new(1000, 500),  // 2.00 sat/vB
        MempoolTransaction::new(1000, 1000), // 4.00 sat/vB - NEW
        MempoolTransaction::new(1000, 2000), // 8.00 sat/vB - NEW
    ];
    snapshots.push(MempoolSnapshot::from_transactions(
        snap2_txs,
        850001,
        base_time + chrono::Duration::minutes(10),
    ));

    // Third snapshot: even more transactions (continued inflow)
    let snap3_txs = vec![
        MempoolTransaction::new(1000, 250),  // 1.00 sat/vB
        MempoolTransaction::new(1000, 500),  // 2.00 sat/vB
        MempoolTransaction::new(1000, 1000), // 4.00 sat/vB
        MempoolTransaction::new(1000, 2000), // 8.00 sat/vB
        MempoolTransaction::new(1000, 4000), // 16.00 sat/vB - NEW
        MempoolTransaction::new(1000, 8000), // 32.00 sat/vB - NEW
    ];
    snapshots.push(MempoolSnapshot::from_transactions(
        snap3_txs,
        850002,
        base_time + chrono::Duration::minutes(20),
    ));

    println!("\n=== Testing Inflow Calculation ===");
    println!("Snapshot 1: 2 transactions");
    println!("Snapshot 2: 4 transactions (+2 inflow)");
    println!("Snapshot 3: 6 transactions (+2 inflow)");

    let estimates = estimator.calculate_estimates(&snapshots, None)?;

    // Check estimates
    println!("\nEstimates with inflow:");
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

    // Now test with DECREASING mempool (outflow/mining)
    let mut snapshots2 = Vec::new();

    // Start with large mempool
    let snap1_txs = vec![
        MempoolTransaction::new(1000, 250),  // 1.00 sat/vB
        MempoolTransaction::new(1000, 500),  // 2.00 sat/vB
        MempoolTransaction::new(1000, 1000), // 4.00 sat/vB
        MempoolTransaction::new(1000, 2000), // 8.00 sat/vB
        MempoolTransaction::new(1000, 4000), // 16.00 sat/vB
        MempoolTransaction::new(1000, 8000), // 32.00 sat/vB
    ];
    snapshots2.push(MempoolSnapshot::from_transactions(
        snap1_txs,
        850010,
        base_time + chrono::Duration::hours(1),
    ));

    // Smaller mempool (transactions mined)
    let snap2_txs = vec![
        MempoolTransaction::new(1000, 250), // 1.00 sat/vB
        MempoolTransaction::new(1000, 500), // 2.00 sat/vB
        MempoolTransaction::new(1000, 1000), // 4.00 sat/vB
                                            // Higher fee transactions were mined
    ];
    snapshots2.push(MempoolSnapshot::from_transactions(
        snap2_txs,
        850011,
        base_time + chrono::Duration::hours(1) + chrono::Duration::minutes(10),
    ));

    println!("\n=== Testing with Outflow (mining) ===");
    println!("Snapshot 1: 6 transactions");
    println!("Snapshot 2: 3 transactions (-3 mined)");

    let estimates2 = estimator.calculate_estimates(&snapshots2, None)?;

    println!("\nEstimates with outflow:");
    for target in [3, 6, 12] {
        if let Some(block_target) = estimates2.estimates.get(&target) {
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
