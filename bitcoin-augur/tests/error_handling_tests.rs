//! Error handling and validation tests for Bitcoin Augur
//!
//! These tests ensure that the library properly validates input
//! and handles error conditions in a way that matches the Kotlin
//! reference implementation.

use bitcoin_augur::{AugurError, FeeEstimator, MempoolSnapshot, MempoolTransaction};
use chrono::{Duration, Utc};

#[test]
fn test_minimum_blocks_validation() {
    // This matches the Kotlin test: "test calculateEstimates throws if numOfBlocks less than 3"
    let estimator = FeeEstimator::new();

    // Create some test snapshots
    let mut snapshots = Vec::new();
    let base_time = Utc::now();

    for i in 0..5 {
        let transactions = vec![
            MempoolTransaction::new(1000, 50),
            MempoolTransaction::new(2000, 100),
            MempoolTransaction::new(1500, 75),
        ];
        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i,
            base_time + Duration::minutes(i as i64 * 10),
        ));
    }

    // Test with num_blocks = 2 (should fail)
    let result = estimator.calculate_estimates(&snapshots, Some(2.0));
    assert!(result.is_err(), "Should fail with num_blocks < 3");

    if let Err(error) = result {
        match error {
            AugurError::InvalidParameter(msg) => {
                assert!(
                    msg.contains("at least 3"),
                    "Error message should mention minimum of 3 blocks"
                );
            }
            _ => panic!("Expected InvalidParameter error"),
        }
    }

    // Test with num_blocks = 3 (should succeed)
    let result = estimator.calculate_estimates(&snapshots, Some(3.0));
    assert!(result.is_ok(), "Should succeed with num_blocks = 3");

    // Test with num_blocks = 100 (should succeed)
    let result = estimator.calculate_estimates(&snapshots, Some(100.0));
    assert!(result.is_ok(), "Should succeed with num_blocks > 3");
}

#[test]
fn test_empty_snapshots_error_handling() {
    let estimator = FeeEstimator::new();

    // Test with empty vector
    let snapshots = vec![];
    let result = estimator.calculate_estimates(&snapshots, None);

    // Should either error or return no estimates
    match result {
        Ok(estimates) => {
            // If it returns Ok, there should be no fee rates
            for target in [3, 6, 12, 24, 144] {
                for confidence in [0.5, 0.8, 0.95] {
                    assert_eq!(
                        estimates.get_fee_rate(target, confidence),
                        None,
                        "Empty snapshots should not produce fee rates"
                    );
                }
            }
        }
        Err(_) => {
            // Error is also acceptable
        }
    }
}

#[test]
fn test_single_snapshot_handling() {
    let estimator = FeeEstimator::new();

    // Create a single snapshot
    let transactions = vec![
        MempoolTransaction::new(1000, 50),
        MempoolTransaction::new(2000, 100),
    ];
    let snapshot = MempoolSnapshot::from_transactions(transactions, 850000, Utc::now());

    let result = estimator.calculate_estimates(&[snapshot], None);

    // Single snapshot might not be enough for estimates
    match result {
        Ok(estimates) => {
            // May or may not have estimates - document the behavior
            let has_estimates = estimates.get_fee_rate(6, 0.95).is_some();
            println!("Single snapshot produces estimates: {}", has_estimates);
        }
        Err(_) => {
            // Error is also acceptable for single snapshot
        }
    }
}

#[test]
fn test_insufficient_data_handling() {
    let estimator = FeeEstimator::new();

    // Create snapshots with very few transactions
    let mut snapshots = Vec::new();
    let base_time = Utc::now();

    for i in 0..3 {
        let transactions = vec![
            MempoolTransaction::new(100, 10), // Very small transaction
        ];
        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i,
            base_time + Duration::minutes(i as i64 * 10),
        ));
    }

    let result = estimator.calculate_estimates(&snapshots, None);

    // With very little data, estimates might be limited or absent
    if let Ok(estimates) = result {
        // Check if we get reasonable estimates despite limited data
        for target in [3, 6, 12, 24, 144] {
            if let Some(fee_rate) = estimates.get_fee_rate(target, 0.95) {
                assert!(
                    fee_rate >= 1.0 && fee_rate <= 10000.0,
                    "Fee rate should be within bounds even with limited data"
                );
            }
        }
    }
}

#[test]
fn test_invalid_confidence_levels() {
    let estimator = FeeEstimator::new();

    // Create valid snapshots with reasonable fee rates
    let mut snapshots = Vec::new();
    let base_time = Utc::now();

    for i in 0..5 {
        let transactions = vec![
            MempoolTransaction::new(1000, 5000),  // 20 sat/vB
            MempoolTransaction::new(2000, 20000), // 40 sat/vB
            MempoolTransaction::new(1500, 11250), // 30 sat/vB
        ];
        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i,
            base_time + Duration::minutes(i as i64 * 10),
        ));
    }

    let estimates = estimator.calculate_estimates(&snapshots, None).unwrap();

    // Test with invalid confidence levels
    assert_eq!(
        estimates.get_fee_rate(6, 0.0),
        None,
        "Confidence 0.0 should return None"
    );

    assert_eq!(
        estimates.get_fee_rate(6, 1.1),
        None,
        "Confidence > 1.0 should return None"
    );

    assert_eq!(
        estimates.get_fee_rate(6, -0.5),
        None,
        "Negative confidence should return None"
    );

    // Valid confidence levels should work
    assert!(
        estimates.get_fee_rate(6, 0.5).is_some() || estimates.get_fee_rate(6, 0.95).is_some(),
        "Valid confidence levels should work"
    );
}

#[test]
fn test_invalid_block_targets() {
    let estimator = FeeEstimator::new();

    // Create valid snapshots
    let mut snapshots = Vec::new();
    let base_time = Utc::now();

    for i in 0..5 {
        let transactions = vec![
            MempoolTransaction::new(1000, 50),
            MempoolTransaction::new(2000, 100),
        ];
        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i,
            base_time + Duration::minutes(i as i64 * 10),
        ));
    }

    let estimates = estimator.calculate_estimates(&snapshots, None).unwrap();

    // Test with invalid block targets
    assert_eq!(
        estimates.get_fee_rate(0, 0.95),
        None,
        "Block target 0 should return None"
    );

    // Very high block targets might or might not be supported
    let high_target_result = estimates.get_fee_rate(10000, 0.95);
    // Document the behavior but don't enforce it
    println!(
        "High block target (10000) supported: {}",
        high_target_result.is_some()
    );
}

#[test]
fn test_unordered_snapshots() {
    let estimator = FeeEstimator::new();

    // Create snapshots with non-sequential timestamps
    let base_time = Utc::now();
    let transactions = vec![
        MempoolTransaction::new(1000, 50),
        MempoolTransaction::new(2000, 100),
    ];

    let snapshots = vec![
        MempoolSnapshot::from_transactions(
            transactions.clone(),
            850002,
            base_time + Duration::minutes(20),
        ),
        MempoolSnapshot::from_transactions(transactions.clone(), 850000, base_time),
        MempoolSnapshot::from_transactions(
            transactions.clone(),
            850001,
            base_time + Duration::minutes(10),
        ),
    ];

    // Should handle unordered snapshots gracefully
    let result = estimator.calculate_estimates(&snapshots, None);
    assert!(
        result.is_ok(),
        "Should handle unordered snapshots without error"
    );
}

#[test]
fn test_duplicate_block_heights() {
    let estimator = FeeEstimator::new();

    // Create snapshots with duplicate block heights
    let base_time = Utc::now();
    let transactions1 = vec![MempoolTransaction::new(1000, 50)];
    let transactions2 = vec![MempoolTransaction::new(2000, 100)];

    let snapshots = vec![
        MempoolSnapshot::from_transactions(transactions1.clone(), 850000, base_time),
        MempoolSnapshot::from_transactions(
            transactions2.clone(),
            850000, // Same block height
            base_time + Duration::minutes(5),
        ),
        MempoolSnapshot::from_transactions(
            transactions1.clone(),
            850001,
            base_time + Duration::minutes(10),
        ),
    ];

    // Should handle duplicate block heights gracefully
    let result = estimator.calculate_estimates(&snapshots, None);
    assert!(
        result.is_ok(),
        "Should handle duplicate block heights without error"
    );
}

#[test]
fn test_negative_fee_rates() {
    // MempoolTransaction should not accept negative fee rates
    // This test documents the behavior

    // In practice, the constructor might panic or clamp to minimum
    // We should ensure consistent behavior

    // Note: The actual API might not expose a way to create negative fee rates
    // This test serves as documentation of the expected behavior
}

#[test]
fn test_overflow_protection() {
    let estimator = FeeEstimator::new();

    // Create snapshots with very large weights
    let mut snapshots = Vec::new();
    let base_time = Utc::now();

    for i in 0..3 {
        let transactions = vec![
            MempoolTransaction::new(u64::MAX / 10, 1000), // Very large weight
            MempoolTransaction::new(u64::MAX / 10, 2000),
        ];
        snapshots.push(MempoolSnapshot::from_transactions(
            transactions,
            850000 + i,
            base_time + Duration::minutes(i as i64 * 10),
        ));
    }

    // Should handle large numbers without overflow
    let result = estimator.calculate_estimates(&snapshots, None);

    match result {
        Ok(estimates) => {
            // If it succeeds, estimates should still be reasonable
            for target in [3, 6, 12] {
                if let Some(fee_rate) = estimates.get_fee_rate(target, 0.95) {
                    assert!(
                        fee_rate >= 1.0 && fee_rate <= 10000.0,
                        "Fee rate should be within bounds even with large weights"
                    );
                }
            }
        }
        Err(_) => {
            // Error is acceptable for extreme values
        }
    }
}
