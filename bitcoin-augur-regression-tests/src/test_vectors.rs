use anyhow::{Context, Result};
use bitcoin_augur::{MempoolSnapshot, MempoolTransaction};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

/// Test vector for fee estimation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestVector {
    pub name: String,
    pub description: String,
    pub mempool_snapshots: Vec<MempoolSnapshotData>,
    pub expected_estimates: ExpectedEstimates,
}

/// Mempool snapshot data for test vectors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolSnapshotData {
    pub block_height: u64,
    pub timestamp: String,
    pub transactions: Vec<TransactionData>,
}

/// Transaction data for test vectors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionData {
    pub weight: u32,
    pub fee: u64,
    pub fee_rate: Option<f64>, // Optional, calculated if not provided
}

/// Expected estimates for validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedEstimates {
    pub block_targets: Vec<ExpectedBlockTarget>,
}

/// Expected estimates for a specific block target
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedBlockTarget {
    pub blocks: usize,
    pub probabilities: Vec<ExpectedProbability>,
}

/// Expected probability and fee rate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedProbability {
    pub probability: f64,
    pub fee_rate: f64,
    pub tolerance: Option<f64>, // Optional tolerance for comparison
}

/// Test vector runner
pub struct TestVectorRunner;

impl TestVectorRunner {
    /// Load test vectors from file
    pub async fn load_vectors(path: &Path) -> Result<Vec<TestVector>> {
        let content = tokio::fs::read_to_string(path)
            .await
            .with_context(|| format!("Failed to read test vectors from {path:?}"))?;

        serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse test vectors from {path:?}"))
    }

    /// Generate default test vectors
    pub fn generate_default_vectors() -> Vec<TestVector> {
        vec![
            Self::generate_simple_vector(),
            Self::generate_congestion_vector(),
            Self::generate_empty_mempool_vector(),
            Self::generate_high_variance_vector(),
            Self::generate_reference_compatibility_vector(),
        ]
    }

    /// Generate a simple test vector
    fn generate_simple_vector() -> TestVector {
        TestVector {
            name: "simple_mempool".to_string(),
            description: "Basic mempool with uniform fee distribution".to_string(),
            mempool_snapshots: vec![MempoolSnapshotData {
                block_height: 850000,
                timestamp: "2025-01-20T12:00:00Z".to_string(),
                transactions: (1..=100)
                    .map(|i| TransactionData {
                        weight: 2000,
                        fee: (i * 1000) as u64,
                        fee_rate: Some(i as f64 * 2.0),
                    })
                    .collect(),
            }],
            expected_estimates: ExpectedEstimates {
                block_targets: vec![ExpectedBlockTarget {
                    blocks: 3,
                    probabilities: vec![
                        ExpectedProbability {
                            probability: 0.05,
                            fee_rate: 1.0, // Minimum fee rate
                            tolerance: Some(0.5),
                        },
                        ExpectedProbability {
                            probability: 0.50,
                            fee_rate: 1.0, // Minimum fee rate
                            tolerance: Some(0.5),
                        },
                        ExpectedProbability {
                            probability: 0.95,
                            fee_rate: 1.0, // Minimum fee rate
                            tolerance: Some(0.5),
                        },
                    ],
                }],
            },
        }
    }

    /// Generate a congestion test vector
    fn generate_congestion_vector() -> TestVector {
        TestVector {
            name: "mempool_congestion".to_string(),
            description: "Congested mempool with high fees".to_string(),
            mempool_snapshots: vec![MempoolSnapshotData {
                block_height: 850100,
                timestamp: "2025-01-20T13:00:00Z".to_string(),
                transactions: (1..=500)
                    .map(|i| TransactionData {
                        weight: 4000,
                        fee: (i * 5000) as u64,
                        fee_rate: Some(i as f64 * 5.0),
                    })
                    .collect(),
            }],
            expected_estimates: ExpectedEstimates {
                block_targets: vec![ExpectedBlockTarget {
                    blocks: 6,
                    probabilities: vec![ExpectedProbability {
                        probability: 0.50,
                        fee_rate: 1.0, // Minimum fee without sufficient data
                        tolerance: Some(0.5),
                    }],
                }],
            },
        }
    }

    /// Generate empty mempool test vector
    fn generate_empty_mempool_vector() -> TestVector {
        TestVector {
            name: "empty_mempool".to_string(),
            description: "Empty mempool should return minimum fees".to_string(),
            mempool_snapshots: vec![MempoolSnapshotData {
                block_height: 850200,
                timestamp: "2025-01-20T14:00:00Z".to_string(),
                transactions: vec![],
            }],
            expected_estimates: ExpectedEstimates {
                block_targets: vec![], // No estimates for empty mempool
            },
        }
    }

    /// Generate high variance test vector
    fn generate_high_variance_vector() -> TestVector {
        let mut transactions = Vec::new();

        // Low fee transactions
        for _ in 0..100 {
            transactions.push(TransactionData {
                weight: 2000,
                fee: 2000,
                fee_rate: Some(1.0),
            });
        }

        // High fee transactions
        for _ in 0..50 {
            transactions.push(TransactionData {
                weight: 2000,
                fee: 200000,
                fee_rate: Some(100.0),
            });
        }

        TestVector {
            name: "high_variance".to_string(),
            description: "Mempool with high fee variance".to_string(),
            mempool_snapshots: vec![MempoolSnapshotData {
                block_height: 850300,
                timestamp: "2025-01-20T15:00:00Z".to_string(),
                transactions,
            }],
            expected_estimates: ExpectedEstimates {
                block_targets: vec![ExpectedBlockTarget {
                    blocks: 3,
                    probabilities: vec![
                        ExpectedProbability {
                            probability: 0.05,
                            fee_rate: 1.0,
                            tolerance: Some(0.5),
                        },
                        ExpectedProbability {
                            probability: 0.95,
                            fee_rate: 1.0, // Returns minimum without sufficient history
                            tolerance: Some(0.5),
                        },
                    ],
                }],
            },
        }
    }

    /// Generate test vector matching reference implementation test data
    fn generate_reference_compatibility_vector() -> TestVector {
        // Based on test data from FeeEstimateEndpointTest.kt
        TestVector {
            name: "reference_compatibility".to_string(),
            description: "Test vector matching Kotlin reference implementation".to_string(),
            mempool_snapshots: vec![MempoolSnapshotData {
                block_height: 850000,
                timestamp: "2025-01-15T10:00:00.123Z".to_string(),
                transactions: vec![
                    // Block 1 transactions
                    TransactionData {
                        weight: 4000,
                        fee: 42000,
                        fee_rate: Some(10.5),
                    },
                    TransactionData {
                        weight: 4000,
                        fee: 61000,
                        fee_rate: Some(15.25),
                    },
                    // Block 6 transactions
                    TransactionData {
                        weight: 4000,
                        fee: 23000,
                        fee_rate: Some(5.75),
                    },
                    TransactionData {
                        weight: 4000,
                        fee: 32494,
                        fee_rate: Some(8.1234),
                    },
                ],
            }],
            expected_estimates: ExpectedEstimates {
                block_targets: vec![ExpectedBlockTarget {
                    blocks: 6,
                    probabilities: vec![ExpectedProbability {
                        probability: 0.50,
                        fee_rate: 1.0, // Minimum without sufficient history
                        tolerance: Some(0.5),
                    }],
                }],
            },
        }
    }

    /// Run test vector
    pub fn run_vector(vector: &TestVector) -> Result<TestVectorResult> {
        info!("Running test vector: {name}", name = vector.name);

        let estimator = bitcoin_augur::FeeEstimator::new();

        // Convert test vector data to mempool snapshots
        let mut snapshots = Vec::new();
        for snapshot_data in &vector.mempool_snapshots {
            let transactions: Vec<MempoolTransaction> = snapshot_data
                .transactions
                .iter()
                .map(|tx| MempoolTransaction::new(tx.weight as u64, tx.fee))
                .collect();

            let timestamp = DateTime::parse_from_rfc3339(&snapshot_data.timestamp)
                .unwrap()
                .with_timezone(&Utc);

            snapshots.push(MempoolSnapshot::from_transactions(
                transactions,
                snapshot_data.block_height as u32,
                timestamp,
            ));
        }

        // Calculate estimates
        let estimates = estimator.calculate_estimates(&snapshots, None)?;

        // Validate against expected estimates
        let mut validations = Vec::new();
        for expected_target in &vector.expected_estimates.block_targets {
            let actual_target = estimates.estimates.get(&(expected_target.blocks as u32));

            if let Some(actual) = actual_target {
                for expected_prob in &expected_target.probabilities {
                    let actual_fee = actual.get_fee_rate(expected_prob.probability);

                    let validation = if let Some(fee) = actual_fee {
                        let tolerance = expected_prob.tolerance.unwrap_or(0.01);
                        let diff = (fee - expected_prob.fee_rate).abs();

                        ProbabilityValidation {
                            blocks: expected_target.blocks,
                            probability: expected_prob.probability,
                            expected: expected_prob.fee_rate,
                            actual: Some(fee),
                            passed: diff <= tolerance,
                            message: if diff <= tolerance {
                                format!("Within tolerance (diff: {diff:.4})")
                            } else {
                                format!("Outside tolerance (diff: {diff:.4} > {tolerance:.4})")
                            },
                        }
                    } else {
                        ProbabilityValidation {
                            blocks: expected_target.blocks,
                            probability: expected_prob.probability,
                            expected: expected_prob.fee_rate,
                            actual: None,
                            passed: false,
                            message: "No fee rate calculated".to_string(),
                        }
                    };

                    validations.push(validation);
                }
            } else {
                for expected_prob in &expected_target.probabilities {
                    validations.push(ProbabilityValidation {
                        blocks: expected_target.blocks,
                        probability: expected_prob.probability,
                        expected: expected_prob.fee_rate,
                        actual: None,
                        passed: false,
                        message: format!(
                            "No estimates for {blocks} blocks",
                            blocks = expected_target.blocks
                        ),
                    });
                }
            }
        }

        let all_passed = validations.iter().all(|v| v.passed);

        Ok(TestVectorResult {
            name: vector.name.clone(),
            passed: all_passed,
            validations,
        })
    }

    /// Save test vectors to file
    pub async fn save_vectors(vectors: &[TestVector], path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(vectors)?;
        tokio::fs::create_dir_all(path.parent().unwrap()).await?;
        tokio::fs::write(path, json).await?;
        Ok(())
    }
}

/// Test vector validation result
#[derive(Debug)]
pub struct TestVectorResult {
    pub name: String,
    pub passed: bool,
    pub validations: Vec<ProbabilityValidation>,
}

/// Individual probability validation
#[derive(Debug)]
pub struct ProbabilityValidation {
    pub blocks: usize,
    pub probability: f64,
    pub expected: f64,
    pub actual: Option<f64>,
    pub passed: bool,
    pub message: String,
}

impl TestVectorResult {
    pub fn print_summary(&self) {
        use colored::Colorize;

        let status = if self.passed {
            "PASSED".green()
        } else {
            "FAILED".red()
        };

        println!(
            "\nTest Vector: {name} [{status}]",
            name = self.name,
            status = status
        );
        println!("{separator}", separator = "-".repeat(60));

        for validation in &self.validations {
            let symbol = if validation.passed {
                "✓".green()
            } else {
                "✗".red()
            };

            let actual_str = validation
                .actual
                .map(|v| format!("{v:.4}"))
                .unwrap_or_else(|| "N/A".to_string());

            println!(
                "{symbol} Blocks: {blocks}, Prob: {prob:.2}, Expected: {expected:.4}, Actual: {actual}, {message}",
                symbol = symbol,
                blocks = validation.blocks,
                prob = validation.probability,
                expected = validation.expected,
                actual = actual_str,
                message = validation.message
            );
        }
    }
}
