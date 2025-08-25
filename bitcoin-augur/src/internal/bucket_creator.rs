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

    // ===== KOTLIN PARITY TESTS =====
    // These tests match BucketCreatorTest from the Kotlin implementation

    #[test]
    fn parity_create_buckets_single_transaction() {
        // Matches Kotlin: "test createFeeRateBuckets with single transaction"
        let tx = MempoolTransaction::new(400, 200); // 100 vBytes (weight/4), 2 sat/vB

        let buckets = create_fee_rate_buckets(&[tx]);

        // Calculate expected bucket index for fee rate of 2 sat/vB
        let expected_bucket_index = (2.0_f64.ln() * 100.0).round() as i32;

        assert!(buckets.contains_key(&expected_bucket_index));
        assert_eq!(buckets[&expected_bucket_index], 400);
    }

    #[test]
    fn parity_create_buckets_multiple_same_bucket() {
        // Matches Kotlin: "test createFeeRateBuckets with multiple transactions in same bucket"
        // Both transactions have 2 sat/vB fee rate
        let tx1 = MempoolTransaction::new(400, 200); // 100 vBytes, 200 sats
        let tx2 = MempoolTransaction::new(800, 400); // 200 vBytes, 400 sats

        let buckets = create_fee_rate_buckets(&[tx1, tx2]);

        let expected_bucket_index = (2.0_f64.ln() * 100.0).round() as i32;

        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[&expected_bucket_index], 1200); // Total weight = 1200
    }

    #[test]
    fn parity_create_buckets_different_buckets() {
        // Matches Kotlin: "test createFeeRateBuckets with transactions in different buckets"
        // 2 sat/vB and 4 sat/vB transactions
        let tx1 = MempoolTransaction::new(400, 200); // 100 vBytes, 2 sat/vB
        let tx2 = MempoolTransaction::new(400, 400); // 100 vBytes, 4 sat/vB

        let buckets = create_fee_rate_buckets(&[tx1, tx2]);

        assert_eq!(buckets.len(), 2);
        assert!(buckets.values().all(|&v| v == 400));
    }

    #[test]
    fn parity_create_buckets_exponential_fee_rates() {
        // Matches Kotlin: "test createFeeRateBuckets with exponential fee rates"
        
        // Create transactions with exponentially increasing fee rates
        let transactions = vec![
            MempoolTransaction::new(400, 100),  // 1 sat/vB
            MempoolTransaction::new(400, 272),  // e sat/vB (2.718...)
            MempoolTransaction::new(400, 739),  // e^2 sat/vB (7.389...)
            MempoolTransaction::new(400, 2009), // e^3 sat/vB (20.085...)
        ];

        let buckets = create_fee_rate_buckets(&transactions);

        // Calculate expected bucket indices
        let expected_buckets: BTreeMap<i32, u64> = [
            (0, 400),   // ln(1) * 100 = 0
            (100, 400), // ln(e) * 100 = 100
            (200, 400), // ln(e^2) * 100 = 200
            (300, 400), // ln(e^3) * 100 = 300
        ].iter().cloned().collect();

        assert_eq!(buckets, expected_buckets);
    }

    #[test]
    fn parity_create_buckets_duplicate_fee_rates() {
        // Matches Kotlin: "test createFeeRateBuckets with duplicate fee rates"
        // Create multiple transactions with the same fee rates
        let transactions = vec![
            MempoolTransaction::new(400, 100), // 1 sat/vB
            MempoolTransaction::new(400, 100), // 1 sat/vB
            MempoolTransaction::new(400, 272), // e sat/vB
            MempoolTransaction::new(400, 272), // e sat/vB
        ];

        let buckets = create_fee_rate_buckets(&transactions);

        // Calculate expected bucket indices
        let expected_buckets: BTreeMap<i32, u64> = [
            (0, 800),   // Two transactions with fee rate 1
            (100, 800), // Two transactions with fee rate e
        ].iter().cloned().collect();

        assert_eq!(buckets, expected_buckets);
    }

    #[test]
    fn parity_create_buckets_very_high_fee_rates() {
        // Matches Kotlin: "test createFeeRateBuckets with very high fee rates"
        let transactions = vec![
            MempoolTransaction::new(400, 1_000_000_000), // Very high fee rate
        ];

        let buckets = create_fee_rate_buckets(&transactions);

        // The bucket index should be at BUCKET_MAX
        assert!(buckets.contains_key(&BUCKET_MAX));
        assert_eq!(buckets[&BUCKET_MAX], 400);
    }
}
