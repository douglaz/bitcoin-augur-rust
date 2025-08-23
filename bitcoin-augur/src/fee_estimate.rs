use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Represents a complete fee estimate with predictions for various block targets
/// and confidence levels.
///
/// This struct contains fee rate estimates organized by confirmation target (in blocks)
/// and confidence level (as a probability between 0.0 and 1.0).
///
/// # Example
/// ```
/// use bitcoin_augur::FeeEstimate;
///
/// // Assuming you have a fee estimate from the estimator
/// # use chrono::Utc;
/// # use std::collections::BTreeMap;
/// # let fee_estimate = FeeEstimate::empty(Utc::now());
///
/// // Get fee rate for confirming within 6 blocks with 95% confidence
/// let fee_rate = fee_estimate.get_fee_rate(6, 0.95);
///
/// // Get all estimates for confirming within 6 blocks
/// let six_block_target = fee_estimate.get_estimates_for_target(6);
///
/// // Print a formatted table of all estimates
/// println!("{}", fee_estimate);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeEstimate {
    /// Map of block targets to their respective BlockTarget estimates
    pub estimates: BTreeMap<u32, BlockTarget>,

    /// When this estimate was calculated
    pub timestamp: DateTime<Utc>,
}

impl FeeEstimate {
    /// Creates a new fee estimate.
    pub fn new(estimates: BTreeMap<u32, BlockTarget>, timestamp: DateTime<Utc>) -> Self {
        Self {
            estimates,
            timestamp,
        }
    }

    /// Creates an empty fee estimate with no estimates available.
    pub fn empty(timestamp: DateTime<Utc>) -> Self {
        Self {
            estimates: BTreeMap::new(),
            timestamp,
        }
    }

    /// Gets the recommended fee rate for a specific target block count and confidence level.
    ///
    /// # Arguments
    /// * `target_blocks` - The desired confirmation target in blocks
    /// * `probability` - The desired confidence level (between 0.0 and 1.0)
    ///
    /// # Returns
    /// The fee rate in sat/vB, or None if the estimate is not available
    pub fn get_fee_rate(&self, target_blocks: u32, probability: f64) -> Option<f64> {
        self.estimates
            .get(&target_blocks)
            .and_then(|target| target.get_fee_rate(probability))
    }

    /// Gets all fee rate estimates for a specific target block count.
    ///
    /// # Arguments
    /// * `target_blocks` - The desired confirmation target in blocks
    ///
    /// # Returns
    /// A BlockTarget containing fee rates for various confidence levels,
    /// or None if no estimates are available for this target
    pub fn get_estimates_for_target(&self, target_blocks: u32) -> Option<&BlockTarget> {
        self.estimates.get(&target_blocks)
    }

    /// Gets the nearest available block target to the requested target.
    ///
    /// This is useful when the exact requested block target is not available.
    ///
    /// # Arguments
    /// * `target_blocks` - The desired confirmation target in blocks
    ///
    /// # Returns
    /// The nearest available block target, or None if no estimates are available
    pub fn get_nearest_block_target(&self, target_blocks: u32) -> Option<u32> {
        if self.estimates.is_empty() {
            return None;
        }

        if self.estimates.contains_key(&target_blocks) {
            return Some(target_blocks);
        }

        self.estimates
            .keys()
            .min_by_key(|&&k| (k as i32 - target_blocks as i32).abs())
            .copied()
    }

    /// Returns all available block targets in ascending order.
    pub fn get_available_block_targets(&self) -> Vec<u32> {
        self.estimates.keys().copied().collect()
    }

    /// Returns all available confidence levels in ascending order.
    pub fn get_available_confidence_levels(&self) -> Vec<f64> {
        let mut levels = std::collections::HashSet::new();
        for target in self.estimates.values() {
            for prob in target.probabilities.keys() {
                levels.insert(prob.0.to_bits()); // Use to_bits on the inner f64
            }
        }
        let mut result: Vec<f64> = levels.iter().map(|&bits| f64::from_bits(bits)).collect();
        result.sort_by(|a, b| a.partial_cmp(b).unwrap());
        result
    }
}

impl fmt::Display for FeeEstimate {
    /// Returns a formatted string representation of the fee estimates as a table.
    ///
    /// The table shows all block targets as rows and all confidence levels as columns.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.estimates.is_empty() {
            return Ok(());
        }

        // Get all unique probabilities
        let probabilities = self.get_available_confidence_levels();

        // Header row
        write!(f, "{:10}", "Blocks")?;
        for prob in &probabilities {
            write!(f, "\t{:10}", format!("{:.1}%", prob * 100.0))?;
        }
        writeln!(f)?;

        // Data rows
        for (blocks, target) in &self.estimates {
            write!(f, "{:10}", blocks)?;
            for prob in &probabilities {
                if let Some(fee_rate) = target.get_fee_rate(*prob) {
                    write!(f, "\t{:10.4}", fee_rate)?;
                } else {
                    write!(f, "\t{:10}", "-")?;
                }
            }
            writeln!(f)?;
        }

        Ok(())
    }
}

/// Represents fee estimates for a specific block target with multiple confidence levels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockTarget {
    /// The confirmation target in blocks
    pub blocks: u32,

    /// Map of confidence levels to their respective fee rates
    /// Key: probability (0.0 to 1.0)
    /// Value: fee rate in sat/vB
    pub probabilities: BTreeMap<OrderedFloat, f64>,
}

impl BlockTarget {
    /// Creates a new block target.
    pub fn new(blocks: u32, probabilities: BTreeMap<OrderedFloat, f64>) -> Self {
        Self {
            blocks,
            probabilities,
        }
    }

    /// Gets the fee rate for a specific confidence level.
    ///
    /// # Arguments
    /// * `probability` - The desired confidence level (between 0.0 and 1.0)
    ///
    /// # Returns
    /// The fee rate in sat/vB, or None if not available for this confidence level
    pub fn get_fee_rate(&self, probability: f64) -> Option<f64> {
        self.probabilities.get(&OrderedFloat(probability)).copied()
    }
}

/// A wrapper around f64 that implements Ord for use in BTreeMap.
/// This is safe for our use case as we only use valid probability values (0.0 to 1.0).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OrderedFloat(pub f64);

impl Eq for OrderedFloat {}

impl PartialOrd for OrderedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .partial_cmp(&other.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_fee_estimate() {
        let estimate = FeeEstimate::empty(Utc::now());
        assert!(estimate.estimates.is_empty());
        assert_eq!(estimate.get_fee_rate(6, 0.95), None);
    }

    #[test]
    fn test_get_fee_rate() {
        let mut probabilities = BTreeMap::new();
        probabilities.insert(OrderedFloat(0.5), 5.0);
        probabilities.insert(OrderedFloat(0.95), 10.0);

        let block_target = BlockTarget::new(6, probabilities);

        let mut estimates = BTreeMap::new();
        estimates.insert(6, block_target);

        let fee_estimate = FeeEstimate::new(estimates, Utc::now());

        assert_eq!(fee_estimate.get_fee_rate(6, 0.5), Some(5.0));
        assert_eq!(fee_estimate.get_fee_rate(6, 0.95), Some(10.0));
        assert_eq!(fee_estimate.get_fee_rate(6, 0.8), None);
        assert_eq!(fee_estimate.get_fee_rate(3, 0.5), None);
    }

    #[test]
    fn test_get_nearest_block_target() {
        let mut estimates = BTreeMap::new();
        estimates.insert(3, BlockTarget::new(3, BTreeMap::new()));
        estimates.insert(6, BlockTarget::new(6, BTreeMap::new()));
        estimates.insert(12, BlockTarget::new(12, BTreeMap::new()));

        let fee_estimate = FeeEstimate::new(estimates, Utc::now());

        assert_eq!(fee_estimate.get_nearest_block_target(6), Some(6));
        assert_eq!(fee_estimate.get_nearest_block_target(7), Some(6));
        assert_eq!(fee_estimate.get_nearest_block_target(10), Some(12));
        assert_eq!(fee_estimate.get_nearest_block_target(1), Some(3));
    }
}
