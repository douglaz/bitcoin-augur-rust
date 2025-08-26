#![no_main]

use libfuzzer_sys::fuzz_target;
use serde_json::Value;

// Fuzz Bitcoin RPC response parsing
fuzz_target!(|data: &[u8]| {
    // Try to parse as JSON
    if let Ok(s) = std::str::from_utf8(data) {
        if let Ok(json) = serde_json::from_str::<Value>(s) {
            // Try to extract fields that RPC client expects
            let _ = json.get("result");
            let _ = json.get("error");
            let _ = json.get("id");
            
            // Try to parse as array (batch response)
            if let Some(array) = json.as_array() {
                for item in array {
                    let _ = item.get("result");
                    let _ = item.get("error");
                    let _ = item.get("id");
                    
                    // Check for blockchain info structure
                    if let Some(result) = item.get("result") {
                        let _ = result.get("blocks");
                        let _ = result.get("bestblockhash");
                        
                        // Check for mempool structure
                        if let Some(obj) = result.as_object() {
                            for (_txid, entry) in obj {
                                let _ = entry.get("vsize");
                                let _ = entry.get("weight");
                                if let Some(fees) = entry.get("fees") {
                                    let _ = fees.get("base");
                                }
                            }
                        }
                    }
                }
            }
        }
    }
});