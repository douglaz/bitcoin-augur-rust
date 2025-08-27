use rand::Rng;
use serde::{Deserialize, Serialize};

/// Test case for regression testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub name: String,
    pub description: String,
    pub mempool_state: MempoolState,
    pub api_calls: Vec<ApiCall>,
}

/// Mempool state for test case
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempoolState {
    pub transactions: Vec<TestTransaction>,
    pub block_height: u64,
}

/// Test transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestTransaction {
    pub weight: u32,
    pub fee: u64,
    pub fee_rate: Option<f64>,
}

/// API call to test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiCall {
    pub method: String,
    pub path: String,
    pub expected_status: u16,
    pub validate_response: bool,
}

/// Test case generator
pub struct TestCaseGenerator;

impl TestCaseGenerator {
    /// Generate test cases
    pub fn generate(count: usize) -> Vec<TestCase> {
        let mut cases = Vec::new();
        let mut rng = rand::thread_rng();

        // Generate various test scenarios
        for i in 0..count {
            let case = match i % 10 {
                0 => Self::generate_empty_mempool(),
                1 => Self::generate_single_transaction(),
                2 => Self::generate_uniform_distribution(&mut rng),
                3 => Self::generate_bimodal_distribution(&mut rng),
                4 => Self::generate_high_fee_spike(&mut rng),
                5 => Self::generate_low_fee_congestion(&mut rng),
                6 => Self::generate_graduated_fees(&mut rng),
                7 => Self::generate_random_distribution(&mut rng),
                8 => Self::generate_large_mempool(&mut rng),
                _ => Self::generate_mixed_weights(&mut rng),
            };
            cases.push(case);
        }

        cases
    }

    /// Generate empty mempool test case
    fn generate_empty_mempool() -> TestCase {
        TestCase {
            name: "empty_mempool".to_string(),
            description: "Test with empty mempool".to_string(),
            mempool_state: MempoolState {
                transactions: vec![],
                block_height: 850000,
            },
            api_calls: vec![
                ApiCall {
                    method: "GET".to_string(),
                    path: "/fees".to_string(),
                    expected_status: 200,
                    validate_response: true,
                },
                ApiCall {
                    method: "GET".to_string(),
                    path: "/fees/target/6".to_string(),
                    expected_status: 200,
                    validate_response: true,
                },
            ],
        }
    }

    /// Generate single transaction test case
    fn generate_single_transaction() -> TestCase {
        TestCase {
            name: "single_transaction".to_string(),
            description: "Mempool with single transaction".to_string(),
            mempool_state: MempoolState {
                transactions: vec![TestTransaction {
                    weight: 2000,
                    fee: 10000,
                    fee_rate: Some(5.0),
                }],
                block_height: 850001,
            },
            api_calls: vec![ApiCall {
                method: "GET".to_string(),
                path: "/fees".to_string(),
                expected_status: 200,
                validate_response: true,
            }],
        }
    }

    /// Generate uniform distribution
    fn generate_uniform_distribution(rng: &mut impl Rng) -> TestCase {
        let base_fee = rng.gen_range(1..50);
        let transactions: Vec<TestTransaction> = (0..100)
            .map(|_| TestTransaction {
                weight: 2000,
                fee: (base_fee * 1000) as u64,
                fee_rate: Some(base_fee as f64),
            })
            .collect();

        TestCase {
            name: format!("uniform_fee_{}", base_fee),
            description: format!("Uniform distribution at {} sat/vB", base_fee),
            mempool_state: MempoolState {
                transactions,
                block_height: 850000 + rng.gen_range(0..1000),
            },
            api_calls: vec![ApiCall {
                method: "GET".to_string(),
                path: "/fees".to_string(),
                expected_status: 200,
                validate_response: true,
            }],
        }
    }

    /// Generate bimodal distribution
    fn generate_bimodal_distribution(rng: &mut impl Rng) -> TestCase {
        let mut transactions = Vec::new();

        // Low fee group
        for _ in 0..50 {
            let fee_rate = rng.gen_range(1..5);
            transactions.push(TestTransaction {
                weight: 2000,
                fee: (fee_rate * 500) as u64,
                fee_rate: Some(fee_rate as f64),
            });
        }

        // High fee group
        for _ in 0..50 {
            let fee_rate = rng.gen_range(50..100);
            transactions.push(TestTransaction {
                weight: 2000,
                fee: (fee_rate * 500) as u64,
                fee_rate: Some(fee_rate as f64),
            });
        }

        TestCase {
            name: "bimodal_distribution".to_string(),
            description: "Bimodal fee distribution".to_string(),
            mempool_state: MempoolState {
                transactions,
                block_height: 850000 + rng.gen_range(0..1000),
            },
            api_calls: vec![
                ApiCall {
                    method: "GET".to_string(),
                    path: "/fees".to_string(),
                    expected_status: 200,
                    validate_response: true,
                },
                ApiCall {
                    method: "GET".to_string(),
                    path: "/fees/target/3".to_string(),
                    expected_status: 200,
                    validate_response: true,
                },
            ],
        }
    }

    /// Generate high fee spike
    fn generate_high_fee_spike(rng: &mut impl Rng) -> TestCase {
        let mut transactions = Vec::new();

        // Normal fees
        for _ in 0..90 {
            let fee_rate = rng.gen_range(1..10);
            transactions.push(TestTransaction {
                weight: 2000,
                fee: (fee_rate * 500) as u64,
                fee_rate: Some(fee_rate as f64),
            });
        }

        // Spike fees
        for _ in 0..10 {
            let fee_rate = rng.gen_range(200..500);
            transactions.push(TestTransaction {
                weight: 2000,
                fee: (fee_rate * 500) as u64,
                fee_rate: Some(fee_rate as f64),
            });
        }

        TestCase {
            name: "high_fee_spike".to_string(),
            description: "Normal fees with high fee spike".to_string(),
            mempool_state: MempoolState {
                transactions,
                block_height: 850000 + rng.gen_range(0..1000),
            },
            api_calls: vec![
                ApiCall {
                    method: "GET".to_string(),
                    path: "/fees".to_string(),
                    expected_status: 200,
                    validate_response: true,
                },
                ApiCall {
                    method: "GET".to_string(),
                    path: "/fees/target/1".to_string(),
                    expected_status: 200,
                    validate_response: true,
                },
            ],
        }
    }

    /// Generate low fee congestion
    fn generate_low_fee_congestion(rng: &mut impl Rng) -> TestCase {
        let transactions: Vec<TestTransaction> = (0..500)
            .map(|_| {
                let fee_rate = rng.gen_range(1..5);
                TestTransaction {
                    weight: rng.gen_range(1000..10000),
                    fee: fee_rate * 500,
                    fee_rate: Some(fee_rate as f64),
                }
            })
            .collect();

        TestCase {
            name: "low_fee_congestion".to_string(),
            description: "Many low fee transactions".to_string(),
            mempool_state: MempoolState {
                transactions,
                block_height: 850000 + rng.gen_range(0..1000),
            },
            api_calls: vec![
                ApiCall {
                    method: "GET".to_string(),
                    path: "/fees".to_string(),
                    expected_status: 200,
                    validate_response: true,
                },
                ApiCall {
                    method: "GET".to_string(),
                    path: "/fees/target/144".to_string(),
                    expected_status: 200,
                    validate_response: true,
                },
            ],
        }
    }

    /// Generate graduated fees
    fn generate_graduated_fees(rng: &mut impl Rng) -> TestCase {
        let transactions: Vec<TestTransaction> = (1..=100)
            .map(|i| TestTransaction {
                weight: 2000,
                fee: (i * 1000) as u64,
                fee_rate: Some(i as f64),
            })
            .collect();

        TestCase {
            name: "graduated_fees".to_string(),
            description: "Linearly increasing fees".to_string(),
            mempool_state: MempoolState {
                transactions,
                block_height: 850000 + rng.gen_range(0..1000),
            },
            api_calls: vec![
                ApiCall {
                    method: "GET".to_string(),
                    path: "/fees".to_string(),
                    expected_status: 200,
                    validate_response: true,
                },
                ApiCall {
                    method: "GET".to_string(),
                    path: "/fees/target/6".to_string(),
                    expected_status: 200,
                    validate_response: true,
                },
            ],
        }
    }

    /// Generate random distribution
    fn generate_random_distribution(rng: &mut impl Rng) -> TestCase {
        let count = rng.gen_range(10..200);
        let transactions: Vec<TestTransaction> = (0..count)
            .map(|_| {
                let weight = rng.gen_range(500..10000);
                let fee_rate = rng.gen_range(1..200) as f64;
                let fee = ((fee_rate * weight as f64) / 4.0) as u64;
                TestTransaction {
                    weight,
                    fee,
                    fee_rate: Some(fee_rate),
                }
            })
            .collect();

        TestCase {
            name: format!("random_{}", count),
            description: format!("Random distribution with {} txs", count),
            mempool_state: MempoolState {
                transactions,
                block_height: 850000 + rng.gen_range(0..1000),
            },
            api_calls: vec![ApiCall {
                method: "GET".to_string(),
                path: "/fees".to_string(),
                expected_status: 200,
                validate_response: true,
            }],
        }
    }

    /// Generate large mempool
    fn generate_large_mempool(rng: &mut impl Rng) -> TestCase {
        let transactions: Vec<TestTransaction> = (0..1000)
            .map(|_| {
                let weight = rng.gen_range(1000..5000);
                let fee_rate = rng.gen_range(1..50) as f64;
                let fee = ((fee_rate * weight as f64) / 4.0) as u64;
                TestTransaction {
                    weight,
                    fee,
                    fee_rate: Some(fee_rate),
                }
            })
            .collect();

        TestCase {
            name: "large_mempool".to_string(),
            description: "Large mempool with 1000 transactions".to_string(),
            mempool_state: MempoolState {
                transactions,
                block_height: 850000 + rng.gen_range(0..1000),
            },
            api_calls: vec![
                ApiCall {
                    method: "GET".to_string(),
                    path: "/fees".to_string(),
                    expected_status: 200,
                    validate_response: true,
                },
                ApiCall {
                    method: "GET".to_string(),
                    path: "/fees/target/12".to_string(),
                    expected_status: 200,
                    validate_response: true,
                },
                ApiCall {
                    method: "GET".to_string(),
                    path: "/fees/target/24".to_string(),
                    expected_status: 200,
                    validate_response: true,
                },
            ],
        }
    }

    /// Generate mixed weights
    fn generate_mixed_weights(rng: &mut impl Rng) -> TestCase {
        let mut transactions = Vec::new();

        // Small transactions
        for _ in 0..30 {
            let weight = rng.gen_range(500..1500);
            let fee_rate = rng.gen_range(5..20) as f64;
            transactions.push(TestTransaction {
                weight,
                fee: ((fee_rate * weight as f64) / 4.0) as u64,
                fee_rate: Some(fee_rate),
            });
        }

        // Medium transactions
        for _ in 0..40 {
            let weight = rng.gen_range(1500..4000);
            let fee_rate = rng.gen_range(3..15) as f64;
            transactions.push(TestTransaction {
                weight,
                fee: ((fee_rate * weight as f64) / 4.0) as u64,
                fee_rate: Some(fee_rate),
            });
        }

        // Large transactions
        for _ in 0..30 {
            let weight = rng.gen_range(4000..10000);
            let fee_rate = rng.gen_range(1..10) as f64;
            transactions.push(TestTransaction {
                weight,
                fee: ((fee_rate * weight as f64) / 4.0) as u64,
                fee_rate: Some(fee_rate),
            });
        }

        TestCase {
            name: "mixed_weights".to_string(),
            description: "Mixed transaction weights".to_string(),
            mempool_state: MempoolState {
                transactions,
                block_height: 850000 + rng.gen_range(0..1000),
            },
            api_calls: vec![ApiCall {
                method: "GET".to_string(),
                path: "/fees".to_string(),
                expected_status: 200,
                validate_response: true,
            }],
        }
    }
}
