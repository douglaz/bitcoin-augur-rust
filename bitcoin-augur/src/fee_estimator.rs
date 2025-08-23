use chrono::{Duration, Utc};
use std::collections::BTreeMap;

use crate::{
    error::{AugurError, Result},
    fee_estimate::{BlockTarget, FeeEstimate, OrderedFloat},
    internal::{FeeCalculator, InflowCalculator, SnapshotArray},
    MempoolSnapshot,
};

/// The main entry point for calculating Bitcoin fee estimates.
///
/// FeeEstimator analyzes historical mempool data to predict transaction confirmation
/// times with various confidence levels.
///
/// # Example
/// ```no_run
/// use bitcoin_augur::{FeeEstimator, MempoolSnapshot, MempoolTransaction};
/// use chrono::Utc;
///
/// // Initialize the estimator with default settings
/// let fee_estimator = FeeEstimator::new();
///
/// // Create mempool snapshots (normally from Bitcoin Core)
/// # let transactions = vec![];
/// let snapshot = MempoolSnapshot::from_transactions(
///     transactions,
///     850000,
///     Utc::now(),
/// );
///
/// // Calculate fee estimates using historical snapshots
/// let historical_snapshots = vec![snapshot]; // Would normally have 24 hours of data
/// let fee_estimate = fee_estimator.calculate_estimates(&historical_snapshots, None)
///     .expect("Failed to calculate estimates");
///
/// // Get fee rate for specific target and confidence
/// if let Some(fee_rate) = fee_estimate.get_fee_rate(6, 0.95) {
///     println!("Recommended fee rate for 6 blocks at 95% confidence: {:.2} sat/vB", fee_rate);
/// }
/// ```
pub struct FeeEstimator {
    probabilities: Vec<f64>,
    block_targets: Vec<f64>,
    short_term_window: Duration,
    long_term_window: Duration,
    calculator: FeeCalculator,
}

impl FeeEstimator {
    /// Default block targets for fee estimation (3, 6, 9, 12, 18, 24, 36, 48, 72, 96, 144 blocks).
    pub const DEFAULT_BLOCK_TARGETS: &'static [f64] = 
        &[3.0, 6.0, 9.0, 12.0, 18.0, 24.0, 36.0, 48.0, 72.0, 96.0, 144.0];
    
    /// Default confidence levels for fee estimation (5%, 20%, 50%, 80%, 95%).
    pub const DEFAULT_PROBABILITIES: &'static [f64] = &[0.05, 0.20, 0.50, 0.80, 0.95];
    
    /// Creates a new FeeEstimator with default settings.
    ///
    /// Default settings:
    /// - Probabilities: 5%, 20%, 50%, 80%, 95%
    /// - Block targets: 3, 6, 9, 12, 18, 24, 36, 48, 72, 96, 144
    /// - Short-term window: 30 minutes
    /// - Long-term window: 24 hours
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Creates a new FeeEstimator with custom settings.
    ///
    /// # Arguments
    /// * `probabilities` - Confidence levels (must be between 0.0 and 1.0)
    /// * `block_targets` - Block confirmation targets (must be positive)
    /// * `short_term_window` - Duration for short-term inflow analysis
    /// * `long_term_window` - Duration for long-term inflow analysis
    pub fn with_config(
        probabilities: Vec<f64>,
        block_targets: Vec<f64>,
        short_term_window: Duration,
        long_term_window: Duration,
    ) -> Result<Self> {
        // Validate inputs
        if probabilities.is_empty() {
            return Err(AugurError::invalid_config("At least one probability level must be provided"));
        }
        if block_targets.is_empty() {
            return Err(AugurError::invalid_config("At least one block target must be provided"));
        }
        if probabilities.iter().any(|&p| !(0.0..=1.0).contains(&p)) {
            return Err(AugurError::invalid_config("All probabilities must be between 0.0 and 1.0"));
        }
        if block_targets.iter().any(|&t| t <= 0.0) {
            return Err(AugurError::invalid_config("All block targets must be positive"));
        }
        
        let calculator = FeeCalculator::new(probabilities.clone(), block_targets.clone());
        
        Ok(Self {
            probabilities,
            block_targets,
            short_term_window,
            long_term_window,
            calculator,
        })
    }
    
    /// Calculates fee estimates based on historical mempool snapshots.
    ///
    /// This method analyzes the provided mempool snapshots to generate fee estimates
    /// for each block target and confidence level.
    ///
    /// # Arguments
    /// * `snapshots` - A slice of historical mempool snapshots, ideally covering
    ///                 at least the past 24 hours.
    /// * `num_blocks` - Optional specific block target to estimate for.
    ///                  If provided, must be at least 3.0 (we can't simulate partial blocks).
    ///
    /// # Returns
    /// A `FeeEstimate` object containing the calculated estimates, or an error if
    /// estimation fails.
    pub fn calculate_estimates(
        &self,
        snapshots: &[MempoolSnapshot],
        num_blocks: Option<f64>,
    ) -> Result<FeeEstimate> {
        // Validate num_blocks if specified
        if let Some(blocks) = num_blocks {
            if blocks < 3.0 {
                return Err(AugurError::invalid_parameter(
                    "num_blocks must be at least 3 if specified"
                ));
            }
        }
        
        if snapshots.is_empty() {
            return Ok(FeeEstimate::empty(Utc::now()));
        }
        
        // Sort snapshots by timestamp
        let mut ordered_snapshots = snapshots.to_vec();
        ordered_snapshots.sort_by_key(|s| s.timestamp);
        
        // Convert to internal array representation
        let snapshot_arrays: Vec<SnapshotArray> = ordered_snapshots
            .iter()
            .map(SnapshotArray::from_snapshot)
            .collect();
        
        // Extract latest mempool weights
        let latest_mempool_weights = &snapshot_arrays.last().unwrap().buckets;
        
        // Calculate inflow rates
        let short_term_inflows = InflowCalculator::calculate_inflows(
            &snapshot_arrays,
            self.short_term_window,
        );
        
        let long_term_inflows = InflowCalculator::calculate_inflows(
            &snapshot_arrays,
            self.long_term_window,
        );
        
        // Use custom calculator if num_blocks is specified
        let (calculator, targets) = if let Some(blocks) = num_blocks {
            let custom_calc = FeeCalculator::new(
                self.probabilities.clone(),
                vec![blocks],
            );
            (custom_calc, vec![blocks])
        } else {
            (
                FeeCalculator::new(self.probabilities.clone(), self.block_targets.clone()),
                self.block_targets.clone(),
            )
        };
        
        // Calculate fee estimates using the core algorithm
        let fee_matrix = calculator.get_fee_estimates(
            latest_mempool_weights,
            &short_term_inflows,
            &long_term_inflows,
        );
        
        // Convert to FeeEstimate structure
        let estimates = self.convert_to_fee_estimate(
            &fee_matrix,
            ordered_snapshots.last().unwrap().timestamp,
            &targets,
        );
        
        Ok(estimates)
    }
    
    /// Converts the raw fee matrix to a structured FeeEstimate object.
    fn convert_to_fee_estimate(
        &self,
        fee_matrix: &ndarray::Array2<Option<f64>>,
        timestamp: chrono::DateTime<chrono::Utc>,
        targets: &[f64],
    ) -> FeeEstimate {
        let mut estimates = BTreeMap::new();
        
        for (block_idx, &mean_blocks) in targets.iter().enumerate() {
            let mut probabilities = BTreeMap::new();
            
            for (prob_idx, &prob) in self.probabilities.iter().enumerate() {
                if let Some(fee_rate) = fee_matrix[[block_idx, prob_idx]] {
                    probabilities.insert(OrderedFloat(prob), fee_rate);
                }
            }
            
            if !probabilities.is_empty() {
                let block_target = BlockTarget::new(mean_blocks as u32, probabilities);
                estimates.insert(mean_blocks as u32, block_target);
            }
        }
        
        FeeEstimate::new(estimates, timestamp)
    }
}

impl Default for FeeEstimator {
    fn default() -> Self {
        let probabilities = Self::DEFAULT_PROBABILITIES.to_vec();
        let block_targets = Self::DEFAULT_BLOCK_TARGETS.to_vec();
        let calculator = FeeCalculator::new(probabilities.clone(), block_targets.clone());
        
        Self {
            probabilities,
            block_targets,
            short_term_window: Duration::minutes(30),
            long_term_window: Duration::hours(24),
            calculator,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MempoolTransaction;
    
    #[test]
    fn test_fee_estimator_creation() {
        let estimator = FeeEstimator::new();
        assert_eq!(estimator.probabilities.len(), 5);
        assert_eq!(estimator.block_targets.len(), 11);
    }
    
    #[test]
    fn test_empty_snapshots() {
        let estimator = FeeEstimator::new();
        let result = estimator.calculate_estimates(&[], None).unwrap();
        assert!(result.estimates.is_empty());
    }
    
    #[test]
    fn test_custom_config() {
        let estimator = FeeEstimator::with_config(
            vec![0.5, 0.95],
            vec![6.0, 12.0],
            Duration::minutes(15),
            Duration::hours(12),
        ).unwrap();
        
        assert_eq!(estimator.probabilities.len(), 2);
        assert_eq!(estimator.block_targets.len(), 2);
    }
    
    #[test]
    fn test_invalid_config() {
        // Empty probabilities
        let result = FeeEstimator::with_config(
            vec![],
            vec![6.0],
            Duration::minutes(30),
            Duration::hours(24),
        );
        assert!(result.is_err());
        
        // Invalid probability
        let result = FeeEstimator::with_config(
            vec![1.5],
            vec![6.0],
            Duration::minutes(30),
            Duration::hours(24),
        );
        assert!(result.is_err());
        
        // Negative block target
        let result = FeeEstimator::with_config(
            vec![0.5],
            vec![-1.0],
            Duration::minutes(30),
            Duration::hours(24),
        );
        assert!(result.is_err());
    }
    
    #[test]
    fn test_num_blocks_validation() {
        let estimator = FeeEstimator::new();
        
        // Create a simple snapshot
        let transactions = vec![
            MempoolTransaction::new(400, 1000),
        ];
        let snapshot = MempoolSnapshot::from_transactions(
            transactions,
            850000,
            Utc::now(),
        );
        
        // num_blocks too small
        let result = estimator.calculate_estimates(&[snapshot.clone()], Some(2.0));
        assert!(result.is_err());
        
        // num_blocks valid
        let result = estimator.calculate_estimates(&[snapshot], Some(6.0));
        assert!(result.is_ok());
    }
}