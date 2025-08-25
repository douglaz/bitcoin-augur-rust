use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use std::collections::HashMap;

/// Test data generator that matches Kotlin TestUtils
pub struct TestDataGenerator;

impl TestDataGenerator {
    /// Port of Kotlin TestUtils.createDefaultBaseWeights()
    /// Creates a distribution of transaction weights at various fee rates
    pub fn create_default_base_weights() -> HashMap<OrderedFloat<f64>, u64> {
        use ordered_float::OrderedFloat;
        let mut weights = HashMap::new();
        let mut rng = rand::thread_rng();

        // Low fee range (0.5 - 4.0 sat/vB)
        for fee in 1..=8 {
            let fee_rate = fee as f64 * 0.5;
            let weight = 500_000 + (rng.gen::<f64>() * 1_500_000.0) as u64;
            weights.insert(OrderedFloat(fee_rate), weight);
        }

        // Medium fee range (4.5 - 16.0 sat/vB) with spikes
        for fee in 9..=32 {
            let fee_rate = fee as f64 * 0.5;
            let base_weight = 2_000_000 + (rng.gen::<f64>() * 5_000_000.0) as u64;
            let weight = match fee_rate as u32 {
                5 => base_weight * 3,  // Spike at 5 sat/vB
                8 => base_weight * 4,  // Major spike at 8 sat/vB
                10 => base_weight * 3, // Spike at 10 sat/vB
                12 => base_weight * 2, // Smaller spike at 12 sat/vB
                15 => base_weight * 3, // Spike at 15 sat/vB
                _ => base_weight,
            };
            weights.insert(OrderedFloat(fee_rate), weight);
        }

        // High fee range (16.5 - 32.0 sat/vB)
        for fee in 33..=64 {
            let fee_rate = fee as f64 * 0.5;
            let base_weight = 1_000_000 + (rng.gen::<f64>() * 3_000_000.0) as u64;
            let weight = match fee_rate as u32 {
                20 => base_weight * 3, // Spike at 20 sat/vB
                25 => base_weight * 4, // Major spike at 25 sat/vB
                30 => base_weight * 2, // Smaller spike at 30 sat/vB
                _ => base_weight,
            };
            weights.insert(OrderedFloat(fee_rate), weight);
        }

        weights
    }

    /// Port of Kotlin TestUtils.createHighInflowRates()
    #[allow(dead_code)]
    pub fn create_high_inflow_rates() -> HashMap<OrderedFloat<f64>, u64> {
        use ordered_float::OrderedFloat;
        let mut rates = HashMap::new();

        for fee in 1..=64 {
            let fee_rate = fee as f64 * 0.5;
            let inflow_rate = if fee_rate <= 4.0 {
                20_000 // ~33 WU/s each
            } else if fee_rate <= 8.0 {
                40_000 // ~67 WU/s each
            } else if fee_rate <= 16.0 {
                80_000 // ~133 WU/s each
            } else {
                2_000_000 // ~167 WU/s each
            };
            rates.insert(OrderedFloat(fee_rate), inflow_rate);
        }

        rates
    }

    /// Port of Kotlin TestUtils.createVeryLowInflowRates()
    #[allow(dead_code)]
    pub fn create_very_low_inflow_rates() -> HashMap<OrderedFloat<f64>, u64> {
        use ordered_float::OrderedFloat;
        let mut rates = HashMap::new();

        for fee in 1..=64 {
            let fee_rate = fee as f64 * 0.5;
            rates.insert(OrderedFloat(fee_rate), 100); // Very low constant rate
        }

        rates
    }

    /// Port of Kotlin TestUtils.createSnapshotSequence()
    /// Generates a sequence of mempool snapshots for testing
    pub fn create_snapshot_sequence(
        block_count: usize,
        snapshots_per_block: usize,
        start_time: DateTime<Utc>,
        inflow_rate_change_time: Option<Duration>,
    ) -> Vec<TestSnapshot> {
        let mut snapshots = Vec::new();
        let base_weights = Self::create_default_base_weights();
        let mut current_block_height = 850000u32;

        for block_idx in 0..block_count {
            // Mine a block every N snapshots
            if block_idx > 0 {
                current_block_height += 1;
            }

            for snap_idx in 0..snapshots_per_block {
                let snapshot_index = block_idx * snapshots_per_block + snap_idx;
                let time = start_time + Duration::minutes((snapshot_index * 10) as i64);

                // Create transactions based on weights
                let mut transactions = Vec::new();
                for (fee_rate, weight) in &base_weights {
                    // Convert fee rate to total fee (sat/vB * weight / 4)
                    let fee = (fee_rate.0 * (*weight as f64) / 4.0) as u64;
                    transactions.push(TestTransaction {
                        weight: *weight,
                        fee,
                        fee_rate: fee_rate.0,
                    });
                }

                // Apply inflow rate changes if specified
                if let Some(change_interval) = inflow_rate_change_time {
                    let elapsed = time - start_time;
                    if elapsed > change_interval {
                        // Modify transaction distribution based on inflow rates
                        // This simulates changing mempool conditions over time
                    }
                }

                snapshots.push(TestSnapshot {
                    block_height: current_block_height,
                    timestamp: time,
                    transactions,
                });
            }
        }

        snapshots
    }

    /// Create a single test transaction
    #[allow(dead_code)]
    pub fn create_transaction(fee_rate: f64, weight: u64) -> TestTransaction {
        let fee = (fee_rate * weight as f64 / 4.0) as u64; // Convert fee rate to total fee
        TestTransaction {
            weight,
            fee,
            fee_rate,
        }
    }
}

/// Test snapshot structure
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TestSnapshot {
    pub block_height: u32,
    pub timestamp: DateTime<Utc>,
    pub transactions: Vec<TestTransaction>,
}

/// Test transaction structure
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TestTransaction {
    pub weight: u64,
    pub fee: u64,
    pub fee_rate: f64,
}

// Need to add ordered-float dependency for HashMap keys
use ordered_float::OrderedFloat;
