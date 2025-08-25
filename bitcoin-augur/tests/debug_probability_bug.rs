use bitcoin_augur::{FeeEstimator, MempoolSnapshot, MempoolTransaction, Result};
use chrono::Utc;

#[test]
fn debug_probability_ordering() -> Result<()> {
    // Create simple test data with a range of fee rates
    let mut transactions = Vec::new();

    // Add transactions at various fee rates (1-100 sat/vB)
    for fee_rate in 1..=100 {
        let weight = 4000; // 1000 vBytes
        let fee = (fee_rate as f64 * weight as f64 / 4.0) as u64;
        transactions.push(MempoolTransaction::new(weight, fee));
    }

    // Create snapshot
    let base_time = Utc::now();
    let snapshot = MempoolSnapshot::from_transactions(transactions, 850000, base_time);

    // Create multiple snapshots to have enough data
    let mut snapshots = vec![snapshot.clone()];
    for i in 1..10 {
        snapshots.push(MempoolSnapshot::from_transactions(
            vec![
                MempoolTransaction::new(4000, 10000), // 10 sat/vB
                MempoolTransaction::new(4000, 20000), // 20 sat/vB
                MempoolTransaction::new(4000, 50000), // 50 sat/vB
            ],
            850000 + i,
            base_time + chrono::Duration::minutes(10 * i as i64),
        ));
    }

    let estimator = FeeEstimator::new();
    let estimates = estimator.calculate_estimates(&snapshots, None)?;

    println!("\n=== Debugging Probability Ordering Bug ===\n");

    // Check all targets
    for target in &[3, 6, 12, 24, 144] {
        println!("Target {} blocks:", target);

        if let Some(block_target) = estimates.estimates.get(target) {
            let mut prev_fee = 0.0;
            let mut violations = Vec::new();

            for prob in &[0.05, 0.20, 0.50, 0.80, 0.95] {
                if let Some(fee) = block_target.get_fee_rate(*prob) {
                    println!("  {:.0}% confidence: {:.2} sat/vB", prob * 100.0, fee);

                    if fee < prev_fee && prev_fee > 0.0 {
                        violations.push(format!(
                            "{:.0}%: {:.2} < {:.2}",
                            prob * 100.0,
                            fee,
                            prev_fee
                        ));
                    }
                    prev_fee = fee;
                } else {
                    println!("  {:.0}% confidence: No estimate", prob * 100.0);
                }
            }

            if !violations.is_empty() {
                println!("  ⚠️  VIOLATIONS: {}", violations.join(", "));
            }
        } else {
            println!("  No estimates available");
        }
        println!();
    }

    // Show the bucket conversion
    println!("Bucket Index to Fee Rate Conversion:");
    for bucket in &[0, 100, 200, 300, 400, 500, 600, 700, 800, 900, 1000] {
        let fee_rate = (*bucket as f64 / 100.0).exp();
        println!("  Bucket {:4}: {:10.2} sat/vB", bucket, fee_rate);
    }

    // The bug: we're getting bucket 0 (= 1.00 sat/vB) for 95% confidence
    // when we should get a higher bucket

    Ok(())
}
