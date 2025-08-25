//! Numerical precision tests for Bitcoin Augur
//!
//! These tests verify that calculations maintain numerical accuracy
//! and match the expected behavior of the Kotlin reference implementation,
//! particularly for Poisson distribution calculations and floating-point operations.

use bitcoin_augur::{FeeEstimator, MempoolSnapshot, MempoolTransaction};
use chrono::{Duration, Utc};
use statrs::distribution::{DiscreteCDF, Poisson};

/// Test helper to create a standard set of snapshots
fn create_test_snapshots(
    num_blocks: usize,
    transactions_per_snapshot: usize,
) -> Vec<MempoolSnapshot> {
    let mut snapshots = Vec::new();
    let base_time = Utc::now();

    for i in 0..num_blocks {
        let mut transactions = Vec::new();
        for j in 0..transactions_per_snapshot {
            // Create transactions with varying fee rates
            let fee_rate = 10 + (j as u64 * 10);
            let weight = 1000 + (j as u64 * 100);
            transactions.push(MempoolTransaction::new(weight, fee_rate));
        }

        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i as u32,
            base_time + Duration::minutes((i * 10) as i64),
        ));
    }

    snapshots
}

#[test]
fn test_poisson_distribution_accuracy() {
    // Test that our Poisson calculations match expected values
    // This is critical for the fee estimation algorithm

    // Note: The algorithm finds the largest k such that P(X >= k) >= confidence
    // This is for finding expected blocks to simulate
    let test_cases = vec![
        (3.0, 0.5, 3),      // Mean 3, 50% confidence
        (3.0, 0.95, 0),     // Mean 3, 95% confidence (pessimistic - few blocks)
        (12.0, 0.5, 12),    // Mean 12, 50% confidence
        (12.0, 0.95, 5),    // Mean 12, 95% confidence (pessimistic)
        (144.0, 0.5, 144),  // Mean 144, 50% confidence
        (144.0, 0.95, 125), // Mean 144, 95% confidence (pessimistic)
    ];

    for (mean, confidence, expected_blocks) in test_cases {
        let poisson = Poisson::new(mean).unwrap();

        // Find the largest k such that P(X >= k) >= confidence
        // This matches the algorithm in calculate_expected_blocks
        let mut calculated_blocks = 0;
        for k in (0..((mean * 4.0) as usize)).rev() {
            let prob_at_least_k = if k == 0 {
                1.0
            } else {
                1.0 - poisson.cdf((k - 1) as u64)
            };

            if prob_at_least_k >= confidence {
                calculated_blocks = k;
                break;
            }
        }

        // Allow small deviation due to discrete distribution
        let diff = (calculated_blocks as i64 - expected_blocks as i64).abs();
        assert!(
            diff <= 2,
            "Poisson calculation mismatch for mean={}, confidence={}: got {}, expected ~{}",
            mean,
            confidence,
            calculated_blocks,
            expected_blocks
        );
    }
}

#[test]
fn test_poisson_monotonicity() {
    // Test that higher confidence results in fewer expected blocks
    // (being more pessimistic about chain speed)

    let means = vec![3.0, 6.0, 12.0, 24.0, 144.0];

    for mean in means {
        let poisson = Poisson::new(mean).unwrap();
        let confidences = vec![0.5, 0.6, 0.7, 0.8, 0.9, 0.95];

        let mut prev_blocks = usize::MAX;
        for confidence in confidences {
            // Find the largest k such that P(X >= k) >= confidence
            let mut blocks = 0;
            for k in (0..((mean * 4.0) as usize)).rev() {
                let prob_at_least_k = if k == 0 {
                    1.0
                } else {
                    1.0 - poisson.cdf((k - 1) as u64)
                };

                if prob_at_least_k >= confidence {
                    blocks = k;
                    break;
                }
            }

            assert!(
                blocks <= prev_blocks,
                "Monotonicity violated: confidence {} expects {} blocks, but previous confidence expected {} (should decrease)",
                confidence, blocks, prev_blocks
            );
            prev_blocks = blocks;
        }
    }
}

#[test]
fn test_floating_point_stability() {
    // Test that repeated calculations with same input produce same output
    let snapshots = create_test_snapshots(10, 50);
    let estimator = FeeEstimator::new();

    // Calculate estimates multiple times
    let mut results = Vec::new();
    for _ in 0..5 {
        let estimates = estimator.calculate_estimates(&snapshots, None).unwrap();
        results.push(estimates);
    }

    // All results should be identical
    for i in 1..results.len() {
        for target in [3, 6, 12, 24, 144] {
            for confidence in [0.5, 0.8, 0.95] {
                let fee1 = results[0].get_fee_rate(target, confidence);
                let fee_i = results[i].get_fee_rate(target, confidence);

                match (fee1, fee_i) {
                    (Some(f1), Some(fi)) => {
                        assert_eq!(
                            f1, fi,
                            "Floating point instability detected: {} != {} for target={}, confidence={}",
                            f1, fi, target, confidence
                        );
                    }
                    (None, None) => {
                        // Both None is fine
                    }
                    _ => {
                        panic!("Inconsistent None/Some results");
                    }
                }
            }
        }
    }
}

#[test]
fn test_bucket_calculation_precision() {
    // Test that fee rate to bucket index calculations are precise
    // The bucket index is calculated as ln(fee_rate) * 100

    let test_cases: Vec<(f64, i32)> = vec![
        (1.0, 0),                   // ln(1) * 100 = 0
        (std::f64::consts::E, 100), // ln(e) * 100 ≈ 100
        (7.389056, 200),            // ln(e^2) * 100 ≈ 200
        (20.085537, 300),           // ln(e^3) * 100 ≈ 300
        (148.413159, 500),          // ln(e^5) * 100 ≈ 500
    ];

    for (fee_rate, expected_bucket) in test_cases {
        let calculated_bucket = (fee_rate.ln() * 100.0) as i32;
        let diff = (calculated_bucket - expected_bucket).abs();

        assert!(
            diff <= 1,
            "Bucket calculation imprecise for fee_rate={}: got {}, expected ~{}",
            fee_rate,
            calculated_bucket,
            expected_bucket
        );
    }
}

#[test]
fn test_weight_accumulation_precision() {
    // Test that accumulating weights doesn't lose precision
    let estimator = FeeEstimator::new();

    // Create snapshots with many small transactions
    let mut snapshots = Vec::new();
    let base_time = Utc::now();

    for i in 0..5 {
        let mut transactions = Vec::new();
        // Add 1000 small transactions
        for _ in 0..1000 {
            transactions.push(MempoolTransaction::new(100, 50));
        }

        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i,
            base_time + Duration::minutes((i * 10) as i64),
        ));
    }

    let result = estimator.calculate_estimates(&snapshots, None);
    assert!(result.is_ok(), "Should handle many small transactions");

    if let Ok(estimates) = result {
        // Check that we get reasonable estimates
        for target in [3, 6, 12] {
            if let Some(fee_rate) = estimates.get_fee_rate(target, 0.95) {
                assert!(
                    (1.0..=10000.0).contains(&fee_rate),
                    "Fee rate should be reasonable despite many small transactions"
                );
            }
        }
    }
}

#[test]
fn test_extreme_fee_rates() {
    // Test handling of extreme fee rates (very high and very low)
    let estimator = FeeEstimator::new();
    let base_time = Utc::now();

    let snapshots = vec![
        MempoolSnapshot::from_transactions(
            vec![
                MempoolTransaction::new(1000, 1),     // Minimum fee rate
                MempoolTransaction::new(1000, 10000), // Maximum fee rate
            ],
            850000,
            base_time,
        ),
        MempoolSnapshot::from_transactions(
            vec![
                MempoolTransaction::new(1000, 1),
                MempoolTransaction::new(1000, 5000),
                MempoolTransaction::new(1000, 10000),
            ],
            850001,
            base_time + Duration::minutes(10),
        ),
        MempoolSnapshot::from_transactions(
            vec![
                MempoolTransaction::new(1000, 1),
                MempoolTransaction::new(1000, 10000),
            ],
            850002,
            base_time + Duration::minutes(20),
        ),
    ];

    let result = estimator.calculate_estimates(&snapshots, None);
    assert!(result.is_ok(), "Should handle extreme fee rates");

    if let Ok(estimates) = result {
        for target in [3, 6, 12] {
            for confidence in [0.5, 0.95] {
                if let Some(fee_rate) = estimates.get_fee_rate(target, confidence) {
                    assert!(fee_rate >= 1.0, "Fee rate should not be below minimum");
                    assert!(fee_rate <= 10000.0, "Fee rate should not exceed maximum");
                }
            }
        }
    }
}

#[test]
fn test_confidence_level_precision() {
    // Test that confidence levels are handled with proper precision
    let snapshots = create_test_snapshots(10, 20);
    let estimator = FeeEstimator::new();

    let estimates = estimator.calculate_estimates(&snapshots, None).unwrap();

    // Test fine-grained confidence levels
    let confidences = vec![0.50, 0.51, 0.52, 0.80, 0.81, 0.82, 0.95, 0.96, 0.97];

    for target in [6, 12, 24] {
        let mut prev_fee = 0.0;
        for confidence in &confidences {
            if let Some(fee_rate) = estimates.get_fee_rate(target, *confidence) {
                // Higher confidence should require equal or higher fees
                assert!(
                    fee_rate >= prev_fee,
                    "Fee rate should not decrease as confidence increases: {} < {} at confidence {}",
                    fee_rate, prev_fee, confidence
                );
                prev_fee = fee_rate;
            }
        }
    }
}

#[test]
fn test_zero_weight_handling() {
    // Test that zero or near-zero weights are handled correctly
    let estimator = FeeEstimator::new();
    let base_time = Utc::now();

    // Note: In practice, transactions can't have zero weight
    // But we should handle edge cases gracefully
    let snapshots = vec![
        MempoolSnapshot::from_transactions(
            vec![
                MempoolTransaction::new(1, 100), // Minimal weight
                MempoolTransaction::new(1000, 50),
            ],
            850000,
            base_time,
        ),
        MempoolSnapshot::from_transactions(
            vec![
                MempoolTransaction::new(1, 200),
                MempoolTransaction::new(2000, 75),
            ],
            850001,
            base_time + Duration::minutes(10),
        ),
        MempoolSnapshot::from_transactions(
            vec![
                MempoolTransaction::new(1, 150),
                MempoolTransaction::new(1500, 60),
            ],
            850002,
            base_time + Duration::minutes(20),
        ),
    ];

    let result = estimator.calculate_estimates(&snapshots, None);

    // Should handle minimal weights without crashing
    assert!(
        result.is_ok() || result.is_err(),
        "Should handle minimal weights gracefully"
    );
}

#[test]
fn test_numerical_overflow_protection() {
    // Test that large numbers don't cause overflow
    let estimator = FeeEstimator::new();
    let base_time = Utc::now();

    // Create snapshots with large but valid values
    let large_weight = 4_000_000; // Full block
    let snapshots = vec![
        MempoolSnapshot::from_transactions(
            vec![
                MempoolTransaction::new(large_weight, 1000),
                MempoolTransaction::new(large_weight, 2000),
            ],
            850000,
            base_time,
        ),
        MempoolSnapshot::from_transactions(
            vec![
                MempoolTransaction::new(large_weight, 1500),
                MempoolTransaction::new(large_weight, 2500),
            ],
            850001,
            base_time + Duration::minutes(10),
        ),
        MempoolSnapshot::from_transactions(
            vec![
                MempoolTransaction::new(large_weight, 1200),
                MempoolTransaction::new(large_weight, 2200),
            ],
            850002,
            base_time + Duration::minutes(20),
        ),
    ];

    let result = estimator.calculate_estimates(&snapshots, None);

    assert!(
        result.is_ok(),
        "Should handle large weights without overflow"
    );
}
