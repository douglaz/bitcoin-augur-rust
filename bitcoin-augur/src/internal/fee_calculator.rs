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
}
