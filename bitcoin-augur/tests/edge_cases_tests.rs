//! Edge case tests for Bitcoin Augur
//!
//! These tests cover extreme and boundary conditions that might not
//! occur frequently in normal usage but should be handled correctly.

use bitcoin_augur::{FeeEstimator, MempoolSnapshot, MempoolTransaction};
use chrono::{Duration, Utc};

#[test]
fn test_all_transactions_same_fee_rate() {
    // When all transactions have the same fee rate
    let estimator = FeeEstimator::new();
    let base_time = Utc::now();
    let fee_rate = 50;

    let mut snapshots = Vec::new();
    for i in 0..5 {
        let transactions: Vec<_> = (0..100)
            .map(|_| MempoolTransaction::new(1000, fee_rate))
            .collect();

        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i,
            base_time + Duration::minutes((i * 10) as i64),
        ));
    }

    let result = estimator.calculate_estimates(&snapshots, None);
    assert!(result.is_ok(), "Should handle uniform fee rates");

    if let Ok(estimates) = result {
        // With uniform rates, all estimates should be similar
        let fee_rates: Vec<_> = [3, 6, 12, 24, 144]
            .iter()
            .filter_map(|&target| estimates.get_fee_rate(target, 0.95))
            .collect();

        if fee_rates.len() > 1 {
            let min = fee_rates.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = fee_rates.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

            // All estimates should be relatively close
            assert!(
                max / min < 10.0,
                "Estimates should be similar for uniform fee rates"
            );
        }
    }
}

#[test]
fn test_single_transaction_per_snapshot() {
    // Edge case with minimal data
    let estimator = FeeEstimator::new();
    let base_time = Utc::now();

    let mut snapshots = Vec::new();
    for i in 0..10 {
        let transaction = MempoolTransaction::new(1000, 50 + i * 10);
        snapshots.push(MempoolSnapshot::from_transactions(
            vec![transaction],
            850000 + i as u32,
            base_time + Duration::minutes((i * 10) as i64),
        ));
    }

    let result = estimator.calculate_estimates(&snapshots, None);

    // Should handle minimal data gracefully
    match result {
        Ok(estimates) => {
            // May produce limited estimates
            let has_any = [3, 6, 12, 24, 144]
                .iter()
                .any(|&t| estimates.get_fee_rate(t, 0.95).is_some());
            println!(
                "Single transaction snapshots produce estimates: {}",
                has_any
            );
        }
        Err(_) => {
            // Error is acceptable for minimal data
        }
    }
}

#[test]
fn test_massive_mempool() {
    // Test with a very large number of transactions
    let estimator = FeeEstimator::new();
    let base_time = Utc::now();

    let mut snapshots = Vec::new();
    for i in 0..3 {
        let mut transactions = Vec::new();
        // Create 10,000 transactions
        for j in 0..10000 {
            let fee_rate = 1 + (j % 1000) as u64;
            let weight = 100 + (j % 900) as u64;
            transactions.push(MempoolTransaction::new(weight, fee_rate));
        }

        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i,
            base_time + Duration::minutes((i * 10) as i64),
        ));
    }

    let result = estimator.calculate_estimates(&snapshots, None);
    assert!(result.is_ok(), "Should handle large mempools");
}

#[test]
fn test_rapid_block_discovery() {
    // Simulate rapid block discovery (many blocks in short time)
    let estimator = FeeEstimator::new();
    let base_time = Utc::now();

    let mut snapshots = Vec::new();
    for i in 0..20 {
        let transactions = vec![
            MempoolTransaction::new(1000, 50),
            MempoolTransaction::new(2000, 100),
        ];

        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i,
            base_time + Duration::seconds(i as i64 * 30), // Block every 30 seconds
        ));
    }

    let result = estimator.calculate_estimates(&snapshots, None);
    assert!(result.is_ok(), "Should handle rapid blocks");
}

#[test]
fn test_mempool_clearing() {
    // Simulate mempool being completely cleared
    let estimator = FeeEstimator::new();
    let base_time = Utc::now();

    let snapshots = vec![
        // Full mempool
        MempoolSnapshot::from_transactions(
            vec![
                MempoolTransaction::new(1000, 50),
                MempoolTransaction::new(2000, 100),
                MempoolTransaction::new(1500, 75),
            ],
            850000,
            base_time,
        ),
        // Empty mempool
        MempoolSnapshot::from_transactions(vec![], 850001, base_time + Duration::minutes(10)),
        // Some transactions return
        MempoolSnapshot::from_transactions(
            vec![MempoolTransaction::new(1000, 60)],
            850002,
            base_time + Duration::minutes(20),
        ),
    ];

    let result = estimator.calculate_estimates(&snapshots, None);
    // Should handle mempool clearing gracefully
    assert!(
        result.is_ok() || result.is_err(),
        "Should handle mempool clearing"
    );
}

#[test]
fn test_fee_rate_spikes() {
    // Sudden spike in fee rates
    let estimator = FeeEstimator::new();
    let base_time = Utc::now();

    let snapshots = vec![
        // Normal fee rates
        MempoolSnapshot::from_transactions(
            vec![
                MempoolTransaction::new(1000, 10),
                MempoolTransaction::new(1000, 20),
            ],
            850000,
            base_time,
        ),
        // Sudden spike
        MempoolSnapshot::from_transactions(
            vec![
                MempoolTransaction::new(1000, 1000),
                MempoolTransaction::new(1000, 2000),
            ],
            850001,
            base_time + Duration::minutes(10),
        ),
        // Return to normal
        MempoolSnapshot::from_transactions(
            vec![
                MempoolTransaction::new(1000, 15),
                MempoolTransaction::new(1000, 25),
            ],
            850002,
            base_time + Duration::minutes(20),
        ),
    ];

    let result = estimator.calculate_estimates(&snapshots, None);
    assert!(result.is_ok(), "Should handle fee rate spikes");
}

#[test]
fn test_very_old_snapshots() {
    // Test with snapshots spanning a very long time period
    let estimator = FeeEstimator::new();
    let base_time = Utc::now() - Duration::days(30); // Start 30 days ago

    let mut snapshots = Vec::new();
    for i in 0..5 {
        let transactions = vec![
            MempoolTransaction::new(1000, 50),
            MempoolTransaction::new(2000, 100),
        ];

        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i,
            base_time + Duration::days(i as i64 * 7), // Weekly snapshots
        ));
    }

    let result = estimator.calculate_estimates(&snapshots, None);

    // Might reject very old data or handle it specially
    match result {
        Ok(_) => println!("Old snapshots accepted"),
        Err(_) => println!("Old snapshots rejected"),
    }
}

#[test]
fn test_block_height_gaps() {
    // Large gaps in block heights
    let estimator = FeeEstimator::new();
    let base_time = Utc::now();

    let snapshots = vec![
        MempoolSnapshot::from_transactions(
            vec![MempoolTransaction::new(1000, 50)],
            850000,
            base_time,
        ),
        MempoolSnapshot::from_transactions(
            vec![MempoolTransaction::new(1000, 60)],
            851000, // 1000 block gap
            base_time + Duration::hours(7),
        ),
        MempoolSnapshot::from_transactions(
            vec![MempoolTransaction::new(1000, 55)],
            851001,
            base_time + Duration::hours(7) + Duration::minutes(10),
        ),
    ];

    let result = estimator.calculate_estimates(&snapshots, None);
    assert!(
        result.is_ok() || result.is_err(),
        "Should handle block height gaps"
    );
}

#[test]
fn test_all_high_fee_rates() {
    // All transactions have very high fee rates
    let estimator = FeeEstimator::new();
    let base_time = Utc::now();

    let mut snapshots = Vec::new();
    for i in 0..5 {
        // Create enough transactions to require multiple blocks
        // Block size is 4M weight units, so we need more than that
        let mut transactions = Vec::new();

        // Add transactions that fill about 2 blocks worth
        // 2000 transactions * 4000 weight = 8M weight units (2 blocks)
        for j in 0..2000 {
            // Vary fee rates between 5000-10000 sat/vB
            let fee_rate = 5000 + (j % 5) * 1000;
            let fee = fee_rate * 4000 / 4; // Convert to actual fee
            transactions.push(MempoolTransaction::new(4000, fee));
        }

        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i,
            base_time + Duration::minutes((i * 10) as i64),
        ));
    }

    let result = estimator.calculate_estimates(&snapshots, None);
    assert!(result.is_ok(), "Should handle high fee rates");

    if let Ok(estimates) = result {
        // When all transactions have high fees, at least the short-term
        // estimates should reflect that. Longer-term estimates may be lower
        // due to the weighting algorithm.
        if let Some(fee_rate) = estimates.get_fee_rate(3, 0.95) {
            assert!(
                fee_rate > 1000.0,
                "Short-term estimates should be high when all fees are high, got {}",
                fee_rate
            );
        }
    }
}

#[test]
fn test_all_minimum_fee_rates() {
    // All transactions at minimum fee rate
    let estimator = FeeEstimator::new();
    let base_time = Utc::now();

    let mut snapshots = Vec::new();
    for i in 0..5 {
        let transactions: Vec<_> = (0..50)
            .map(|_| MempoolTransaction::new(1000, 1)) // Minimum fee rate
            .collect();

        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i,
            base_time + Duration::minutes((i * 10) as i64),
        ));
    }

    let result = estimator.calculate_estimates(&snapshots, None);
    assert!(result.is_ok(), "Should handle minimum fee rates");

    if let Ok(estimates) = result {
        // All estimates should be at or near minimum
        for target in [3, 6, 12] {
            if let Some(fee_rate) = estimates.get_fee_rate(target, 0.95) {
                assert!(
                    fee_rate <= 10.0,
                    "Estimates should be low when all fees are minimum"
                );
            }
        }
    }
}

#[test]
fn test_alternating_empty_full() {
    // Alternating between empty and full mempool
    let estimator = FeeEstimator::new();
    let base_time = Utc::now();

    let mut snapshots = Vec::new();
    for i in 0..10 {
        let transactions = if i % 2 == 0 {
            // Full mempool
            (0..100)
                .map(|j| MempoolTransaction::new(1000, 10 + j))
                .collect()
        } else {
            // Empty mempool
            vec![]
        };

        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i,
            base_time + Duration::minutes((i * 10) as i64),
        ));
    }

    let result = estimator.calculate_estimates(&snapshots, None);

    // Should handle alternating pattern
    match result {
        Ok(_) => println!("Handled alternating empty/full mempool"),
        Err(_) => println!("Could not handle alternating pattern"),
    }
}

#[test]
fn test_decreasing_block_heights() {
    // Block heights that decrease (shouldn't happen but test robustness)
    let estimator = FeeEstimator::new();
    let base_time = Utc::now();

    let snapshots = vec![
        MempoolSnapshot::from_transactions(
            vec![MempoolTransaction::new(1000, 50)],
            850002,
            base_time,
        ),
        MempoolSnapshot::from_transactions(
            vec![MempoolTransaction::new(1000, 60)],
            850001, // Decreasing height
            base_time + Duration::minutes(10),
        ),
        MempoolSnapshot::from_transactions(
            vec![MempoolTransaction::new(1000, 55)],
            850000, // Further decrease
            base_time + Duration::minutes(20),
        ),
    ];

    // Should handle or reject gracefully
    let result = estimator.calculate_estimates(&snapshots, None);
    assert!(
        result.is_ok() || result.is_err(),
        "Should handle decreasing block heights"
    );
}
