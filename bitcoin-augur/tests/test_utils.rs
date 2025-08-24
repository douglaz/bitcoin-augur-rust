use bitcoin_augur::{MempoolSnapshot, MempoolTransaction};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;

/// Port of Kotlin TestUtils for exact parity testing
pub struct TestUtils;

impl TestUtils {
    /// Creates a snapshot with given parameters
    /// Port of Kotlin: TestUtils.createSnapshot
    pub fn create_snapshot(
        block_height: u32,
        timestamp: DateTime<Utc>,
        transactions: Vec<MempoolTransaction>,
    ) -> MempoolSnapshot {
        MempoolSnapshot::from_transactions(transactions, block_height, timestamp)
    }

    /// Creates a transaction with specified fee rate and weight
    /// Port of Kotlin: TestUtils.createTransaction
    pub fn create_transaction(fee_rate: f64, weight: u64) -> MempoolTransaction {
        // Convert fee rate (sat/vB) to total fee
        // fee_rate is in sat/vB, weight is in weight units
        // 1 vByte = 4 weight units
        let fee = (fee_rate * weight as f64 / 4.0) as u64;
        MempoolTransaction::new(weight, fee)
    }

    /// Creates default base weights distribution
    /// Port of Kotlin: TestUtils.createDefaultBaseWeights
    pub fn create_default_base_weights() -> HashMap<String, u64> {
        let mut weights = HashMap::new();

        // Use deterministic values instead of random for test parity
        // Low fee range (0.5 - 4.0 sat/vB)
        for fee in 1..=8 {
            let fee_rate = fee as f64 * 0.5;
            // Use deterministic formula instead of random
            let weight = 500_000 + (fee as u64 * 100_000);
            weights.insert(format!("{:.1}", fee_rate), weight);
        }

        // Medium fee range (4.5 - 16.0 sat/vB) with spikes
        for fee in 9..=32 {
            let fee_rate = fee as f64 * 0.5;
            let base_weight = 2_000_000 + (fee as u64 * 150_000);
            let weight = match fee_rate {
                5.0 => base_weight * 3,  // Spike at 5 sat/vB
                8.0 => base_weight * 4,  // Major spike at 8 sat/vB
                10.0 => base_weight * 3, // Spike at 10 sat/vB
                12.0 => base_weight * 2, // Smaller spike at 12 sat/vB
                15.0 => base_weight * 3, // Spike at 15 sat/vB
                _ => base_weight,
            };
            weights.insert(format!("{:.1}", fee_rate), weight);
        }

        // High fee range (16.5 - 32.0 sat/vB)
        for fee in 33..=64 {
            let fee_rate = fee as f64 * 0.5;
            let base_weight = 1_000_000 + (fee as u64 * 50_000);
            let weight = match fee_rate {
                20.0 => base_weight * 3, // Spike at 20 sat/vB
                25.0 => base_weight * 4, // Major spike at 25 sat/vB
                30.0 => base_weight * 2, // Smaller spike at 30 sat/vB
                _ => base_weight,
            };
            weights.insert(format!("{:.1}", fee_rate), weight);
        }

        weights
    }

    /// Creates high inflow rates
    /// Port of Kotlin: TestUtils.createHighInflowRates
    pub fn create_high_inflow_rates() -> HashMap<String, u64> {
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
            rates.insert(format!("{:.1}", fee_rate), inflow_rate);
        }

        rates
    }

    /// Creates very low inflow rates
    /// Port of Kotlin: TestUtils.createVeryLowInflowRates
    pub fn create_very_low_inflow_rates() -> HashMap<String, u64> {
        let mut rates = HashMap::new();

        for fee in 1..=64 {
            let fee_rate = fee as f64 * 0.5;
            let inflow_rate = if fee_rate <= 4.0 {
                5_000
            } else if fee_rate <= 8.0 {
                10_000
            } else if fee_rate <= 16.0 {
                20_000
            } else {
                25_000
            };
            rates.insert(format!("{:.1}", fee_rate), inflow_rate);
        }

        rates
    }

    /// Creates low inflow rates
    /// Port of Kotlin: TestUtils.createLowInflowRates
    pub fn create_low_inflow_rates() -> HashMap<String, u64> {
        let mut rates = HashMap::new();

        for fee in 1..=64 {
            let fee_rate = fee as f64 * 0.5;
            let inflow_rate = if fee_rate <= 4.0 {
                10_000 // ~17 WU/s each
            } else if fee_rate <= 8.0 {
                20_000 // ~33 WU/s each
            } else if fee_rate <= 16.0 {
                40_000 // ~67 WU/s each
            } else {
                50_000 // ~83 WU/s each
            };
            rates.insert(format!("{:.1}", fee_rate), inflow_rate);
        }

        rates
    }

    /// Creates a sequence of snapshots modeling mempool behavior
    /// Port of Kotlin: TestUtils.createSnapshotSequence
    pub fn create_snapshot_sequence(
        start_time: DateTime<Utc>,
        block_count: usize,
        snapshots_per_block: usize,
        base_weights: HashMap<String, u64>,
        short_term_inflow_rates: HashMap<String, u64>,
        long_term_inflow_rates: HashMap<String, u64>,
        inflow_rate_change_time: Duration,
    ) -> Vec<MempoolSnapshot> {
        let mut snapshots = Vec::new();

        // Calculate end time first (matching Kotlin logic)
        let end_time = start_time + Duration::seconds(600 * (block_count as i64 - 1));

        for block_index in 0..block_count {
            let block_height = 100 + block_index as u32;
            let block_start_time = start_time + Duration::seconds(600 * block_index as i64);
            let snapshot_interval = 600 / snapshots_per_block as i64;

            // Create multiple snapshots for this block
            for snapshot_index in 0..snapshots_per_block {
                let snapshot_time =
                    block_start_time + Duration::seconds(snapshot_interval * snapshot_index as i64);

                // Calculate time from end (matching Kotlin)
                let time_until_end = end_time - snapshot_time;

                // Build transactions based on weights and inflow
                let mut transactions = Vec::new();
                let mut fee_rates: Vec<_> = base_weights.keys().collect();
                fee_rates.sort();

                for fee_rate_str in fee_rates {
                    let fee_rate: f64 = fee_rate_str.parse().unwrap_or(0.0);
                    let base_weight = base_weights.get(fee_rate_str).copied().unwrap_or(0);

                    // Determine which inflow rate to use based on time
                    let inflow_rate = if time_until_end > inflow_rate_change_time {
                        long_term_inflow_rates
                            .get(fee_rate_str)
                            .copied()
                            .unwrap_or(0)
                    } else {
                        short_term_inflow_rates
                            .get(fee_rate_str)
                            .copied()
                            .unwrap_or(0)
                    };

                    // Calculate cumulative weight
                    let elapsed_intervals =
                        (snapshot_time - start_time).num_seconds() as f64 / 600.0;
                    let cumulative_weight =
                        base_weight + (inflow_rate as f64 * elapsed_intervals) as u64;

                    if cumulative_weight > 0 {
                        transactions.push(Self::create_transaction(fee_rate, cumulative_weight));
                    }
                }

                snapshots.push(Self::create_snapshot(
                    block_height,
                    snapshot_time,
                    transactions,
                ));
            }
        }

        snapshots
    }

    /// Convenience method matching Kotlin's default parameters
    pub fn create_snapshot_sequence_default(
        block_count: usize,
        snapshots_per_block: usize,
    ) -> Vec<MempoolSnapshot> {
        let start_time = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        Self::create_snapshot_sequence(
            start_time,
            block_count,
            snapshots_per_block,
            Self::create_default_base_weights(),
            Self::create_high_inflow_rates(),
            Self::create_low_inflow_rates(),
            Duration::hours(1),
        )
    }

    /// Create snapshot sequence with custom inflow rates
    pub fn create_snapshot_sequence_with_inflow(
        block_count: usize,
        snapshots_per_block: usize,
        short_term_inflow_rates: HashMap<String, u64>,
        long_term_inflow_rates: HashMap<String, u64>,
    ) -> Vec<MempoolSnapshot> {
        let start_time = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        Self::create_snapshot_sequence(
            start_time,
            block_count,
            snapshots_per_block,
            Self::create_default_base_weights(),
            short_term_inflow_rates,
            long_term_inflow_rates,
            Duration::hours(1),
        )
    }
}
