use serde::{Deserialize, Serialize};

/// Represents a transaction in the Bitcoin mempool.
///
/// This struct contains the minimal information needed for fee estimation:
/// the transaction's weight and the fee amount.
///
/// # Example
/// ```
/// use bitcoin_augur::MempoolTransaction;
///
/// let transaction = MempoolTransaction {
///     weight: 565,  // Transaction weight in weight units
///     fee: 1000,    // Fee amount in satoshis
/// };
///
/// // Get fee rate in sat/vB
/// let fee_rate = transaction.fee_rate();
/// assert_eq!(fee_rate, 7.079646017699115); // ~7.08 sat/vB
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MempoolTransaction {
    /// The transaction weight in weight units (WU)
    pub weight: u64,

    /// The transaction fee in satoshis
    pub fee: u64,
}

impl MempoolTransaction {
    /// Creates a new mempool transaction.
    pub fn new(weight: u64, fee: u64) -> Self {
        Self { weight, fee }
    }

    /// Calculates the transaction's fee rate in sat/vB.
    ///
    /// This converts from weight units to virtual bytes and calculates
    /// the fee rate as satoshis per virtual byte.
    ///
    /// # Returns
    /// The fee rate in sat/vB, or 0.0 if weight is 0
    pub fn fee_rate(&self) -> f64 {
        if self.weight == 0 {
            return 0.0;
        }
        (self.fee as f64) * WU_PER_BYTE / (self.weight as f64)
    }
}

/// Conversion factor from weight units to virtual bytes.
///
/// A virtual byte (vB) is defined as weight / 4.
/// 1 vB = 4 weight units (WU)
pub const WU_PER_BYTE: f64 = 4.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fee_rate_calculation() {
        let tx = MempoolTransaction::new(400, 1000);
        assert_eq!(tx.fee_rate(), 10.0); // 1000 * 4 / 400 = 10 sat/vB
    }

    #[test]
    fn test_fee_rate_with_zero_weight() {
        let tx = MempoolTransaction::new(0, 1000);
        assert_eq!(tx.fee_rate(), 0.0);
    }

    #[test]
    fn test_fee_rate_precision() {
        let tx = MempoolTransaction::new(565, 1000);
        let fee_rate = tx.fee_rate();
        assert!((fee_rate - 7.079646).abs() < 0.000001);
    }
}
