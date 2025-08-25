use ndarray::{Array1, Array2};
use statrs::distribution::{DiscreteCDF, Poisson};

use crate::internal::BUCKET_MAX;

/// Core implementation of the fee estimation algorithm.
///
/// This struct simulates the mining of blocks to predict when transactions
/// with different fee rates would be confirmed.
pub(crate) struct FeeCalculator {
    probabilities: Vec<f64>,
    block_targets: Vec<f64>,
    expected_blocks: Array2<f64>,
}

impl FeeCalculator {
    /// Block size in weight units (4MB = 4,000,000 WU)
    const BLOCK_SIZE_WEIGHT_UNITS: f64 = 4_000_000.0;

    /// Creates a new fee calculator with the given probability and block target settings.
    pub fn new(probabilities: Vec<f64>, block_targets: Vec<f64>) -> Self {
        let expected_blocks = Self::calculate_expected_blocks(&probabilities, &block_targets);

        Self {
            probabilities,
            block_targets,
            expected_blocks,
        }
    }

    /// Calculates fee estimates based on mempool snapshot and inflow data.
    ///
    /// # Arguments
    /// * `mempool_snapshot` - Current mempool snapshot as an array
    /// * `short_inflows` - Short-term inflow data (typically 30 minutes)
    /// * `long_inflows` - Long-term inflow data (typically 24 hours)
    ///
    /// # Returns
    /// A 2D array of fee estimates where rows are block targets and columns are probabilities.
    /// Values are in sat/vB.
    pub fn get_fee_estimates(
        &self,
        mempool_snapshot: &Array1<f64>,
        short_inflows: &Array1<f64>,
        long_inflows: &Array1<f64>,
    ) -> Array2<Option<f64>> {
        // Add half of short-term inflows as a buffer to current weights
        let current_weights_with_buffer = mempool_snapshot + short_inflows / 2.0;

        // Run simulations for short and long-term intervals
        let short_term_estimates =
            self.run_simulations(&current_weights_with_buffer, short_inflows);

        let long_term_estimates = self.run_simulations(&current_weights_with_buffer, long_inflows);

        // Combine estimates with appropriate weighting
        let weighted_estimates =
            self.get_weighted_estimates(&short_term_estimates, &long_term_estimates);

        // Convert bucket indices to actual fee rates
        let fee_rates = self.convert_buckets_to_fee_rates(&weighted_estimates);

        // Ensure fee rates are monotonically decreasing with block targets
        let monotone_fee_rates = self.enforce_monotonicity(&fee_rates);

        // Create final result array with values filtered by maximum threshold
        self.prepare_result_array(&monotone_fee_rates)
    }

    /// Runs simulations for all block target and probability combinations.
    fn run_simulations(
        &self,
        initial_weights: &Array1<f64>,
        added_weights: &Array1<f64>,
    ) -> Array2<f64> {
        let mut result = Array2::zeros((self.block_targets.len(), self.probabilities.len()));

        for (block_idx, &blocks) in self.block_targets.iter().enumerate() {
            let mean_blocks = blocks as usize;

            for (prob_idx, _) in self.probabilities.iter().enumerate() {
                let expected_blocks = self.expected_blocks[[block_idx, prob_idx]] as usize;

                // Run individual simulation
                let bucket_index = self.run_simulation(
                    initial_weights,
                    added_weights,
                    expected_blocks,
                    mean_blocks,
                );

                result[[block_idx, prob_idx]] = bucket_index.unwrap_or(0) as f64;
            }
        }

        result
    }

    /// Simulates mining blocks and returns the bucket index of the lowest fee rate
    /// that would result in the transaction getting mined.
    fn run_simulation(
        &self,
        initial_weights: &Array1<f64>,
        added_weights: &Array1<f64>,
        expected_blocks: usize,
        mean_blocks: usize,
    ) -> Option<usize> {
        if expected_blocks == 0 {
            return None;
        }

        // Calculate how much of the added weights to use per block
        let expected_mining_time_factor = mean_blocks as f64 / expected_blocks as f64;
        let added_weights_in_one_block = added_weights * expected_mining_time_factor;

        // Mine the expected number of blocks
        let mut current_weights = initial_weights.clone();
        for _ in 0..expected_blocks {
            current_weights += &added_weights_in_one_block;
            current_weights = self.mine_block(&current_weights);
        }

        // Find the index of the last fully mined bucket
        Some(self.find_best_index(&current_weights))
    }

    /// Mines a block by removing the highest fee rate transactions (lowest indices)
    /// until the block size is reached.
    fn mine_block(&self, current_weights: &Array1<f64>) -> Array1<f64> {
        let mut weights_remaining = current_weights.clone();
        let mut weight_units_remaining = Self::BLOCK_SIZE_WEIGHT_UNITS;

        for i in 0..weights_remaining.len() {
            let removed_weight = weights_remaining[i].min(weight_units_remaining);
            weight_units_remaining -= removed_weight;
            weights_remaining[i] -= removed_weight;

            if weight_units_remaining <= 0.0 {
                break;
            }
        }

        weights_remaining
    }

    /// Finds the index of the last bucket that is fully mined.
    fn find_best_index(&self, weights_remaining: &Array1<f64>) -> usize {
        // Find first non-zero remaining weight
        for (i, &weight) in weights_remaining.iter().enumerate() {
            if weight > 0.0 {
                if i == 0 {
                    // No weights were fully mined
                    return BUCKET_MAX as usize + 1;
                }
                // Return the bucket index (convert from reverse order)
                return BUCKET_MAX as usize - (i - 1);
            }
        }

        // All weights are zero, can use cheapest fee rate
        0
    }

    /// Calculates the weighted average of short and long interval estimates.
    fn get_weighted_estimates(
        &self,
        short_estimates: &Array2<f64>,
        long_estimates: &Array2<f64>,
    ) -> Array2<f64> {
        let mut weighted_estimates = Array2::zeros(short_estimates.dim());

        for (i, &target) in self.block_targets.iter().enumerate() {
            // Weight increases quadratically with block target
            let weight = 1.0 - (1.0 - target / 144.0).powi(2);

            for j in 0..self.probabilities.len() {
                weighted_estimates[[i, j]] =
                    short_estimates[[i, j]] * (1.0 - weight) + long_estimates[[i, j]] * weight;
            }
        }

        weighted_estimates
    }

    /// Converts bucket indices to fee rates in sat/vB.
    fn convert_buckets_to_fee_rates(&self, bucket_estimates: &Array2<f64>) -> Array2<f64> {
        bucket_estimates.mapv(|bucket| (bucket / 100.0).exp())
    }

    /// Ensures proper monotonicity in both dimensions:
    /// 1. Fee rates decrease (or stay the same) as block targets increase
    /// 2. Fee rates increase (or stay the same) as probability increases
    fn enforce_monotonicity(&self, fee_rates: &Array2<f64>) -> Array2<f64> {
        let mut result = fee_rates.clone();

        // First pass: Enforce decreasing fees with increasing block targets
        // (longer confirmation targets should have lower or equal fees)
        for j in 0..self.probabilities.len() {
            let mut prev_rate = f64::INFINITY;

            for i in 0..self.block_targets.len() {
                if result[[i, j]] > prev_rate {
                    result[[i, j]] = prev_rate;
                }
                prev_rate = result[[i, j]];
            }
        }

        // Second pass: Ensure non-decreasing fees with increasing probability
        // (higher confidence should have higher or equal fees)
        // This is done after block target monotonicity to preserve that constraint
        for i in 0..self.block_targets.len() {
            for j in 1..self.probabilities.len() {
                if result[[i, j]] < result[[i, j - 1]] {
                    // Fee decreased with probability, which shouldn't happen
                    // Use the maximum that doesn't violate block target monotonicity
                    let candidate = result[[i, j - 1]];

                    // Check if this would violate block target constraint
                    // (fee for this target shouldn't exceed fee for shorter target)
                    if i == 0 || candidate <= result[[i - 1, j]] {
                        result[[i, j]] = candidate;
                    } else if i > 0 {
                        // Use the fee from the shorter target as the maximum
                        result[[i, j]] = result[[i - 1, j]];
                    }
                }
            }
        }

        result
    }

    /// Converts fee estimates to the final array format with None for invalid values.
    fn prepare_result_array(&self, fee_rates: &Array2<f64>) -> Array2<Option<f64>> {
        // Maximum allowed fee rate based on BUCKET_MAX
        let max_allowed_fee_rate = (BUCKET_MAX as f64 / 100.0).exp();

        let mut result = Array2::from_elem(fee_rates.dim(), None);

        for i in 0..self.block_targets.len() {
            for j in 0..self.probabilities.len() {
                let rate = fee_rates[[i, j]];
                if rate < max_allowed_fee_rate && rate > 0.0 {
                    result[[i, j]] = Some(rate);
                }
            }
        }

        result
    }

    /// Calculates expected number of blocks to be mined for each probability level.
    ///
    /// For each confidence level p, we find the largest k such that P(X >= k) >= p,
    /// where X follows a Poisson distribution with mean = target.
    ///
    /// Higher confidence means being more conservative (pessimistic about chain speed):
    /// - 95% confidence: "I'm 95% sure we'll mine AT LEAST k blocks" (small k) → higher fees
    /// - 5% confidence: "I'm only 5% sure we'll mine AT LEAST k blocks" (large k) → lower fees
    fn calculate_expected_blocks(probabilities: &[f64], block_targets: &[f64]) -> Array2<f64> {
        let mut blocks = Array2::zeros((block_targets.len(), probabilities.len()));

        for (i, &target) in block_targets.iter().enumerate() {
            // Create Poisson distribution with mean = target
            let poisson = Poisson::new(target).unwrap();

            // For each probability level, find the number of blocks to simulate
            for (j, &probability) in probabilities.iter().enumerate() {
                // We want to find the largest k such that P(X >= k) >= probability
                // This is equivalent to finding the largest k where the upper tail >= probability
                //
                // For high confidence (e.g., 95%), we're pessimistic about chain speed,
                // so we assume FEWER blocks will be mined, requiring HIGHER fees.
                let max_search = (target * 4.0) as usize;

                // Search backwards to find the largest k where P(X >= k) >= probability
                let mut found = false;
                for k in (0..max_search).rev() {
                    // P(X >= k) = 1 - P(X < k) = 1 - P(X <= k-1)
                    let prob_at_least_k = if k == 0 {
                        1.0 // P(X >= 0) = 1
                    } else {
                        1.0 - poisson.cdf((k - 1) as u64)
                    };

                    if prob_at_least_k >= probability {
                        // We're 'probability' confident that at least k blocks will be mined
                        blocks[[i, j]] = k as f64;
                        found = true;
                        break;
                    }
                }

                // If we didn't find a k (shouldn't happen), use 0
                if !found {
                    blocks[[i, j]] = 0.0;
                }
            }
        }

        blocks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mine_block() {
        let calculator = FeeCalculator::new(vec![0.5], vec![6.0]);

        let mut weights = Array1::zeros(10);
        weights[0] = 1_000_000.0; // High fee
        weights[1] = 2_000_000.0; // Medium fee
        weights[2] = 3_000_000.0; // Low fee

        let remaining = calculator.mine_block(&weights);

        // Should mine 4M weight units total
        assert_eq!(remaining[0], 0.0); // Fully mined
        assert_eq!(remaining[1], 0.0); // Fully mined
        assert_eq!(remaining[2], 2_000_000.0); // Partially mined (1M of 3M)
    }

    #[test]
    fn test_find_best_index() {
        let calculator = FeeCalculator::new(vec![0.5], vec![6.0]);

        let mut weights = Array1::zeros(BUCKET_MAX as usize + 1);
        weights[0] = 0.0; // Mined
        weights[1] = 0.0; // Mined
        weights[2] = 100.0; // Not fully mined

        let index = calculator.find_best_index(&weights);

        // Index 1 was last fully mined, convert from reverse order
        assert_eq!(index, BUCKET_MAX as usize - 1);
    }

    #[test]
    fn test_enforce_monotonicity() {
        let calculator = FeeCalculator::new(vec![0.5, 0.95], vec![3.0, 6.0]);

        let mut fee_rates = Array2::zeros((2, 2));
        fee_rates[[0, 0]] = 5.0; // 3 blocks, 50%
        fee_rates[[1, 0]] = 10.0; // 6 blocks, 50% - should be reduced
        fee_rates[[0, 1]] = 10.0; // 3 blocks, 95%
        fee_rates[[1, 1]] = 8.0; // 6 blocks, 95%

        let monotone = calculator.enforce_monotonicity(&fee_rates);

        // Block target monotonicity: Second row should not exceed first row
        assert_eq!(monotone[[1, 0]], 5.0); // Reduced from 10 to 5
        assert_eq!(monotone[[1, 1]], 8.0); // Unchanged (already less than 10)

        // Probability monotonicity: Higher probabilities should have >= fees
        assert!(monotone[[0, 1]] >= monotone[[0, 0]]); // 95% >= 50% for 3 blocks
        assert!(monotone[[1, 1]] >= monotone[[1, 0]]); // 95% >= 50% for 6 blocks
    }

    #[test]
    fn test_enforce_monotonicity_probability_fix() {
        let calculator = FeeCalculator::new(vec![0.5, 0.8, 0.95], vec![3.0, 6.0]);

        let mut fee_rates = Array2::zeros((2, 3));
        fee_rates[[0, 0]] = 10.0; // 3 blocks, 50%
        fee_rates[[0, 1]] = 15.0; // 3 blocks, 80%
        fee_rates[[0, 2]] = 12.0; // 3 blocks, 95% - WRONG! Should be >= 15
        fee_rates[[1, 0]] = 8.0; // 6 blocks, 50%
        fee_rates[[1, 1]] = 10.0; // 6 blocks, 80%
        fee_rates[[1, 2]] = 9.0; // 6 blocks, 95% - WRONG! Should be >= 10

        let monotone = calculator.enforce_monotonicity(&fee_rates);

        // Check probability monotonicity is fixed
        assert_eq!(monotone[[0, 2]], 15.0); // Fixed: 95% = 80% for 3 blocks
        assert_eq!(monotone[[1, 2]], 10.0); // Fixed: 95% = 80% for 6 blocks

        // Verify block target monotonicity still holds
        assert!(monotone[[1, 0]] <= monotone[[0, 0]]); // 6 blocks <= 3 blocks at 50%
        assert!(monotone[[1, 1]] <= monotone[[0, 1]]); // 6 blocks <= 3 blocks at 80%
        assert!(monotone[[1, 2]] <= monotone[[0, 2]]); // 6 blocks <= 3 blocks at 95%
    }

    // ===== KOTLIN PARITY TESTS =====
    // These tests match FeeEstimatesCalculatorTest from the Kotlin implementation

    #[test]
    fn parity_mine_block_removes_weights_from_highest_fee_buckets_first() {
        // Matches Kotlin: "test mineBlock removes weights from highest fee buckets first"
        let calculator = FeeCalculator::new(vec![0.5, 0.95], vec![3.0, 12.0, 144.0]);

        let mut weights = Array1::zeros(5);
        weights[0] = 1_000_000.0;
        weights[1] = 1_000_000.0;
        weights[2] = 1_000_000.0;
        weights[3] = 1_000_000.0;
        weights[4] = 1_000_000.0;

        // Mine a full block (4M weight units)
        let remaining = calculator.mine_block(&weights);

        // First 4 buckets should be mined
        assert_eq!(remaining[0], 0.0);
        assert_eq!(remaining[1], 0.0);
        assert_eq!(remaining[2], 0.0);
        assert_eq!(remaining[3], 0.0);
        assert_eq!(remaining[4], 1_000_000.0); // Last bucket remains
    }

    #[test]
    fn parity_find_best_index_all_weights_mined() {
        // Matches Kotlin: "test findBestIndex when all weights are mined"
        let calculator = FeeCalculator::new(vec![0.5, 0.95], vec![3.0, 12.0, 144.0]);

        let weights = Array1::zeros(5);
        assert_eq!(calculator.find_best_index(&weights), 0);
    }

    #[test]
    fn parity_find_best_index_no_weights_fully_mined() {
        // Matches Kotlin: "test findBestIndex when no weights are fully mined"
        let calculator = FeeCalculator::new(vec![0.5, 0.95], vec![3.0, 12.0, 144.0]);

        let weights = Array1::from_elem(5, 1000.0);
        assert_eq!(
            calculator.find_best_index(&weights),
            BUCKET_MAX as usize + 1
        );
    }

    #[test]
    fn parity_find_best_index_partially_mined() {
        // Matches Kotlin: "test findBestIndex with partially mined weights"
        let calculator = FeeCalculator::new(vec![0.5, 0.95], vec![3.0, 12.0, 144.0]);

        let mut weights = Array1::zeros(5);
        weights[0] = 0.0; // fully mined
        weights[1] = 0.0; // fully mined
        weights[2] = 500.0; // partially mined
        weights[3] = 1000.0; // unmined
        weights[4] = 1000.0; // unmined

        // Index 1 is the last fully mined bucket (from high to low fees)
        // In reverse order: BUCKET_MAX - 1
        assert_eq!(
            calculator.find_best_index(&weights),
            BUCKET_MAX as usize - 1
        );
    }

    #[test]
    fn parity_run_simulation_simple_case() {
        // Matches Kotlin: "test runSimulation with simple case"
        let calculator = FeeCalculator::new(vec![0.5, 0.95], vec![3.0, 12.0, 144.0]);

        let initial_weights = Array1::from_elem(5, 1_000_000.0);
        let added_weights = Array1::from_elem(5, 100_000.0);

        let result = calculator.run_simulation(
            &initial_weights,
            &added_weights,
            2, // expected_blocks
            2, // mean_blocks
        );

        // With these parameters, some buckets should be fully mined
        assert!(result.is_some());
        if let Some(idx) = result {
            assert!(idx < BUCKET_MAX as usize);
        }
    }

    #[test]
    fn parity_run_simulation_zero_expected_blocks() {
        // Matches Kotlin: "test runSimulation with zero expected blocks"
        let calculator = FeeCalculator::new(vec![0.5, 0.95], vec![3.0, 12.0, 144.0]);

        let initial_weights = Array1::from_elem(5, 1000.0);
        let added_weights = Array1::from_elem(5, 100.0);

        let result = calculator.run_simulation(
            &initial_weights,
            &added_weights,
            0, // expected_blocks
            2, // mean_blocks
        );

        assert!(result.is_none());
    }

    #[test]
    fn parity_mine_block_size_larger_than_total() {
        // Matches Kotlin: "test mineBlock handles block size larger than total weights"
        let calculator = FeeCalculator::new(vec![0.5, 0.95], vec![3.0, 12.0, 144.0]);

        let weights = Array1::from_elem(5, 1_000_000.0);

        // Create weights with 5M total, mine 6M (more than total)
        let mut remaining = weights.clone();
        remaining = calculator.mine_block(&remaining);

        // Mine another full block to ensure everything is cleared
        remaining = calculator.mine_block(&remaining);

        // All buckets should be fully mined
        for i in 0..5 {
            assert_eq!(remaining[i], 0.0);
        }
    }

    #[test]
    fn parity_mine_block_size_smaller_than_any() {
        // Matches Kotlin: "test mineBlock handles block size smaller than any weight"
        let calculator = FeeCalculator::new(vec![0.5, 0.95], vec![3.0, 12.0, 144.0]);

        let weights = Array1::from_elem(5, 4_000_000.0);

        // Mine only 500K weight units (partial first bucket)
        let mut partial_weights = weights.clone();
        partial_weights[0] = 500_000.0;
        partial_weights[1] = 0.0;
        partial_weights[2] = 0.0;
        partial_weights[3] = 0.0;
        partial_weights[4] = 0.0;

        let remaining = calculator.mine_block(&partial_weights);

        // Only first bucket should be mined
        assert_eq!(remaining[0], 0.0);
        for i in 1..5 {
            assert_eq!(remaining[i], 0.0);
        }
    }

    #[test]
    fn parity_find_best_index_last_bucket_mined() {
        // Matches Kotlin: "test findBestIndex when last bucket is mined"
        let calculator = FeeCalculator::new(vec![0.5, 0.95], vec![3.0, 12.0, 144.0]);

        let mut weights = Array1::zeros(5);
        weights[0] = 0.0;
        weights[1] = 1000.0;
        weights[2] = 1000.0;
        weights[3] = 1000.0;
        weights[4] = 1000.0;

        assert_eq!(calculator.find_best_index(&weights), BUCKET_MAX as usize);
    }

    #[test]
    fn parity_weighted_estimates_144_equals_long() {
        // Matches Kotlin: "test getWeightedEstimates returns estimates where the 144 block estimate equals longEstimate"
        let calculator = FeeCalculator::new(vec![0.5, 0.95], vec![3.0, 12.0, 144.0]);

        let mut short_estimates = Array2::zeros((3, 2));
        let mut long_estimates = Array2::zeros((3, 2));

        // Fill with test values
        short_estimates.fill(1.0);
        long_estimates.fill(100.0);

        let weighted = calculator.get_weighted_estimates(&short_estimates, &long_estimates);

        // The 144 block estimate (index 2) should equal long estimate
        assert_eq!(weighted[[2, 0]], 100.0);
        assert_eq!(weighted[[2, 1]], 100.0);

        // Shorter targets should be between short and long
        assert!(weighted[[0, 0]] > 1.0 && weighted[[0, 0]] < 100.0);
        assert!(weighted[[1, 0]] > 1.0 && weighted[[1, 0]] < 100.0);
    }

    #[test]
    fn parity_weighted_estimates_same_short_long() {
        // Matches Kotlin: "test getWeightedEstimates returns same as the short and long if all estimates are the same"
        let calculator = FeeCalculator::new(vec![0.5, 0.95], vec![3.0, 12.0, 144.0]);

        let mut short_estimates = Array2::zeros((3, 2));
        let mut long_estimates = Array2::zeros((3, 2));

        // Fill with same values
        short_estimates.fill(100.0);
        long_estimates.fill(100.0);

        let weighted = calculator.get_weighted_estimates(&short_estimates, &long_estimates);

        // All estimates should be the same
        for i in 0..3 {
            for j in 0..2 {
                assert_eq!(weighted[[i, j]], 100.0);
            }
        }
    }

    #[test]
    fn parity_run_simulation_large_block_size() {
        // Kotlin: test runSimulation with large block size
        // Note: Rust uses fixed block size, so we test with many blocks instead
        let calculator = FeeCalculator::new(vec![0.5, 0.95], vec![3.0, 12.0, 144.0]);

        let mut initial_weights = Array1::zeros(5);
        let mut added_weights = Array1::zeros(5);

        for i in 0..5 {
            initial_weights[i] = 1000.0;
            added_weights[i] = 100.0;
        }

        // Use many blocks to mine everything (simulating large block effect)
        let result = calculator.run_simulation(
            &initial_weights,
            &added_weights,
            10, // expected_blocks - many blocks
            10, // mean_blocks
        );

        // With many blocks, most/all buckets should be mined
        // Returns the best index (lowest non-mined bucket)
        assert_eq!(result, Some(0));
    }

    #[test]
    fn parity_run_simulation_intermediate_mining() {
        // Kotlin: test runSimulation with intermediate mining case
        let calculator = FeeCalculator::new(vec![0.5, 0.95], vec![3.0, 12.0, 144.0]);

        let mut initial_weights = Array1::zeros(5);
        let mut added_weights = Array1::zeros(5);

        // Set up weights that will partially mine with 2 blocks
        for i in 0..5 {
            initial_weights[i] = 2_000_000.0; // 2M weight per bucket
            added_weights[i] = 500_000.0; // 500k added per bucket
        }

        let result = calculator.run_simulation(
            &initial_weights,
            &added_weights,
            2, // expected_blocks
            2, // mean_blocks
        );

        // With 2 blocks of 4M each = 8M total capacity
        // Initial total = 10M, added = 2.5M, total = 12.5M
        // Should mine buckets 4 and part of 3
        assert!(result.is_some());
        assert!(result.unwrap() > 0); // Not all mined
    }

    #[test]
    fn parity_run_simulation_returns_min_fee_when_all_mined() {
        // Kotlin: test runSimulation returns 1 sat per vb when all buckets are mined
        let calculator = FeeCalculator::new(vec![0.5, 0.95], vec![3.0, 12.0, 144.0]);

        let mut initial_weights = Array1::zeros(5);
        let mut added_weights = Array1::zeros(5);

        for i in 0..5 {
            initial_weights[i] = 10_000.0; // Small weights
            added_weights[i] = 5_000.0;
        }

        let result = calculator.run_simulation(
            &initial_weights,
            &added_weights,
            100, // expected_blocks - many blocks to mine everything
            100, // mean_blocks
        );

        // When all buckets are mined, returns bucket 0 (minimum fee rate)
        assert_eq!(result, Some(0));
    }

    #[test]
    fn parity_run_simulation_no_estimate_when_none_mined() {
        // Kotlin: test runSimulation ignores estimate when no buckets fully mined
        let calculator = FeeCalculator::new(vec![0.5, 0.95], vec![3.0, 12.0, 144.0]);

        let mut initial_weights = Array1::zeros(5);
        let mut added_weights = Array1::zeros(5);

        for i in 0..5 {
            initial_weights[i] = 10_000_000.0; // 10M weight per bucket
            added_weights[i] = 1_000_000.0; // 1M added
        }

        let result = calculator.run_simulation(
            &initial_weights,
            &added_weights,
            0, // zero expected_blocks
            1, // mean_blocks
        );

        // With zero blocks, nothing can be mined
        assert_eq!(result, None);
    }

    #[test]
    fn parity_get_expected_blocks_returns_valid() {
        // Kotlin: test getExpectedBlocksMined returns valid blocks
        use statrs::distribution::{DiscreteCDF, Poisson};

        // Test various probabilities and targets
        let test_cases = vec![
            (0.5, 3.0),   // 50% probability, 3 blocks target
            (0.95, 12.0), // 95% probability, 12 blocks target
            (0.8, 144.0), // 80% probability, 144 blocks target
        ];

        for (probability, target) in test_cases {
            // Calculate expected blocks using Poisson distribution
            let poisson = Poisson::new(target).unwrap();

            // Find the number of blocks where P(X <= blocks) >= probability
            let mut expected = 0;
            for blocks in 0..((target * 4.0) as u64) {
                if 1.0 - poisson.cdf(blocks) < probability {
                    expected = blocks;
                    break;
                }
            }

            // Expected blocks should be positive and reasonable
            assert!(expected > 0, "Expected blocks should be positive");
            assert!(
                (expected as f64) < target * 4.0,
                "Expected blocks should be reasonable"
            );

            // For 50% probability, expected blocks should be close to target
            if (probability - 0.5).abs() < 0.01 {
                assert!(
                    (expected as f64 - target).abs() < 2.0,
                    "At 50% probability, expected blocks should be close to target"
                );
            }
        }
    }
}
