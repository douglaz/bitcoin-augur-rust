//! Bitcoin Augur - A Bitcoin fee estimation library
//!
//! This library provides accurate fee estimates by analyzing historical mempool data.
//! It uses statistical modeling to predict confirmation times with various confidence levels.
//!
//! # Features
//! - Predictions based on historical mempool data and transaction inflows
//! - Confidence-based fee rate estimates
//! - Multiple confirmation targets (from 3 to 144 blocks)
//!
//! # Example
//! ```no_run
//! use bitcoin_augur::{FeeEstimator, MempoolSnapshot, MempoolTransaction};
//! use chrono::Utc;
//!
//! // Initialize the estimator with default settings
//! let fee_estimator = FeeEstimator::new();
//!
//! // Create a mempool snapshot from current transactions
//! let transactions = vec![
//!     MempoolTransaction::new(565, 1000),
//!     MempoolTransaction::new(400, 800),
//! ];
//!
//! let snapshot = MempoolSnapshot::from_transactions(
//!     transactions,
//!     850000,  // current block height
//!     Utc::now(),
//! );
//!
//! // Calculate fee estimates using historical snapshots
//! let historical_snapshots = vec![snapshot]; // Would normally have 24 hours of data
//! let fee_estimate = fee_estimator.calculate_estimates(&historical_snapshots, None)
//!     .expect("Failed to calculate estimates");
//!
//! // Get fee rate for specific target and confidence
//! if let Some(fee_rate) = fee_estimate.get_fee_rate(6, 0.95) {
//!     println!("Recommended fee rate for 6 blocks at 95% confidence: {:.2} sat/vB", fee_rate);
//! }
//! ```

// Public modules
pub mod error;

// Data structures
mod fee_estimate;
mod fee_estimator;
mod mempool_snapshot;
mod mempool_transaction;

// Internal implementation modules
pub(crate) mod internal;

// Public exports
pub use error::{AugurError, Result};
pub use fee_estimate::{BlockTarget, FeeEstimate, OrderedFloat};
pub use fee_estimator::FeeEstimator;
pub use mempool_snapshot::MempoolSnapshot;
pub use mempool_transaction::{MempoolTransaction, WU_PER_BYTE};
