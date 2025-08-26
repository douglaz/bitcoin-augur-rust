#![no_main]

use libfuzzer_sys::fuzz_target;
use bitcoin_augur::{FeeEstimator, MempoolTransaction, MempoolSnapshot};
use chrono::Utc;

fuzz_target!(|data: &[u8]| {
    // Ensure we have enough data to create meaningful inputs
    if data.len() < 16 {
        return;
    }

    // Parse fuzzer input to create transactions
    let mut transactions = Vec::new();
    let mut i = 0;
    
    while i + 16 <= data.len() && transactions.len() < 10000 {
        // Extract weight and fee from fuzzer data
        let weight = u64::from_le_bytes([
            data[i], data[i+1], data[i+2], data[i+3],
            data[i+4], data[i+5], data[i+6], data[i+7],
        ]);
        let fee = u64::from_le_bytes([
            data[i+8], data[i+9], data[i+10], data[i+11],
            data[i+12], data[i+13], data[i+14], data[i+15],
        ]);
        
        // Constrain values to reasonable ranges to avoid overflow/OOM
        let weight = (weight % 1_000_000).max(1);
        let fee = fee % 100_000_000; // Max 1 BTC in fees
        
        transactions.push(MempoolTransaction::new(weight, fee));
        i += 16;
    }
    
    if transactions.is_empty() {
        return;
    }
    
    // Create snapshot with fuzzed transactions
    let snapshot = MempoolSnapshot::from_transactions(
        transactions,
        850000,
        Utc::now(),
    );
    
    // Test fee estimation with various confidence levels
    let fee_estimator = FeeEstimator::new();
    let snapshots = vec![snapshot];
    
    // This should not panic regardless of input
    let _ = fee_estimator.calculate_estimates(&snapshots, None);
    let _ = fee_estimator.calculate_estimates(&snapshots, Some(6.0));
    let _ = fee_estimator.calculate_estimates(&snapshots, Some(144.0));
});