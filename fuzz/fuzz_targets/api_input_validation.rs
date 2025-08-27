#![no_main]

use libfuzzer_sys::fuzz_target;

// Fuzz the API endpoint parameter validation
fuzz_target!(|data: &[u8]| {
    if data.len() < 8 {
        return;
    }
    
    // Test num_blocks parameter validation
    let num_blocks = f64::from_le_bytes([
        data[0], data[1], data[2], data[3],
        data[4], data[5], data[6], data[7],
    ]);
    
    // This mimics the validation from get_fee_for_target
    let _is_valid = num_blocks > 0.0 && num_blocks <= 1000.0 && num_blocks.is_finite();
    
    // Test various edge cases
    if !num_blocks.is_nan() && !num_blocks.is_infinite() {
        let _ = num_blocks.floor();
        let _ = num_blocks.ceil();
        let _ = num_blocks.round();
    }
    
    // Test timestamp parameter parsing
    if data.len() >= 16 {
        let timestamp = i64::from_le_bytes([
            data[8], data[9], data[10], data[11],
            data[12], data[13], data[14], data[15],
        ]);
        
        // This should handle any timestamp value without panicking
        use chrono::DateTime;
        let _ = DateTime::from_timestamp(timestamp, 0);
    }
});