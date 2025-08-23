use crate::mempool_transaction::MempoolTransaction;
use std::collections::BTreeMap;

/// Maximum bucket index.
pub const BUCKET_MAX: i32 = 1000;

/// Creates a bucket map from mempool transactions where the key is the bucket index
/// and the value is the sum of the weights at that fee rate, normalized to a one block duration.
///
/// This function groups transactions by their fee rate into logarithmic buckets,
/// providing more precision in the lower fee levels.
pub fn create_fee_rate_buckets(transactions: &[MempoolTransaction]) -> BTreeMap<i32, u64> {
    let mut buckets: BTreeMap<i32, u64> = BTreeMap::new();

    for tx in transactions {
        let fee_rate = tx.fee_rate();
        if fee_rate > 0.0 {
            let bucket_index = calculate_bucket_index(fee_rate);
            *buckets.entry(bucket_index).or_insert(0) += tx.weight;
        }
    }

    buckets
}

/// Calculates bucket index using logarithms, providing more precision in the lower fee levels.
///
/// The formula is: min(round(ln(fee_rate) * 100), BUCKET_MAX)
///
/// This matches the Kotlin implementation's logarithmic bucketing.
fn calculate_bucket_index(fee_rate: f64) -> i32 {
    if fee_rate <= 0.0 {
        return 0;
    }

    let index = (fee_rate.ln() * 100.0).round() as i32;
    index.min(BUCKET_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_bucket_index() {
        // Test edge cases
        assert_eq!(calculate_bucket_index(0.0), 0);
        assert_eq!(calculate_bucket_index(-1.0), 0);

        // Test normal values
        assert_eq!(calculate_bucket_index(1.0), 0); // ln(1) = 0
        assert_eq!(calculate_bucket_index(std::f64::consts::E), 100); // ln(e) = 1

        // Test that we cap at BUCKET_MAX
        assert_eq!(calculate_bucket_index(1e10), BUCKET_MAX);
    }

    #[test]
    fn test_create_fee_rate_buckets() {
        let transactions = vec![
            MempoolTransaction::new(400, 1000), // 10 sat/vB -> bucket ~230
            MempoolTransaction::new(400, 1000), // Same fee rate
            MempoolTransaction::new(600, 600),  // 4 sat/vB -> bucket ~139
        ];

        let buckets = create_fee_rate_buckets(&transactions);

        // Should have 2 buckets (different fee rates)
        assert_eq!(buckets.len(), 2);

        // Check that weights are summed for same bucket
        let bucket_10_satvb = calculate_bucket_index(10.0);
        assert_eq!(buckets.get(&bucket_10_satvb), Some(&800)); // 400 + 400

        let bucket_4_satvb = calculate_bucket_index(4.0);
        assert_eq!(buckets.get(&bucket_4_satvb), Some(&600));
    }

    #[test]
    fn test_empty_transactions() {
        let transactions = vec![];
        let buckets = create_fee_rate_buckets(&transactions);
        assert!(buckets.is_empty());
    }

    #[test]
    fn test_zero_fee_transactions() {
        let transactions = vec![
            MempoolTransaction::new(400, 0), // 0 fee
        ];

        let buckets = create_fee_rate_buckets(&transactions);
        assert!(buckets.is_empty()); // Zero fee transactions are excluded
    }
}
