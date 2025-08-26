#![no_main]

use libfuzzer_sys::fuzz_target;
use bitcoin_augur::MempoolSnapshot;

fuzz_target!(|data: &[u8]| {
    // Try to parse arbitrary data as JSON snapshot
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = serde_json::from_str::<MempoolSnapshot>(s);
    }
    
    // Also test with valid JSON structure but fuzzed values
    if data.len() >= 8 {
        let block_height = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let timestamp = i64::from_le_bytes([
            data.get(4).copied().unwrap_or(0),
            data.get(5).copied().unwrap_or(0),
            data.get(6).copied().unwrap_or(0),
            data.get(7).copied().unwrap_or(0),
            0, 0, 0, 0,
        ]);
        
        let json = format!(
            r#"{{
                "block_height": {},
                "timestamp": {},
                "fee_levels": {{}}
            }}"#,
            block_height,
            timestamp
        );
        
        let _ = serde_json::from_str::<MempoolSnapshot>(&json);
    }
});