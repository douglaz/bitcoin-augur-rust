use bitcoin_augur::{BlockTarget, FeeEstimate};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Response format for fee estimation API matching Kotlin implementation
#[derive(Debug, Serialize, Deserialize)]
pub struct FeeEstimateResponse {
    /// ISO 8601 formatted timestamp of when the mempool was last updated
    #[serde(rename = "mempool_update_time")]
    pub mempool_update_time: String,

    /// Map of block targets to their probability estimates
    pub estimates: BTreeMap<String, BlockTargetResponse>,
}

/// Block target with probability-based fee estimates
#[derive(Debug, Serialize, Deserialize)]
pub struct BlockTargetResponse {
    /// Map of probability percentages to fee rates
    pub probabilities: BTreeMap<String, ProbabilityResponse>,
}

/// Fee rate for a specific probability
#[derive(Debug, Serialize, Deserialize)]
pub struct ProbabilityResponse {
    /// Fee rate in satoshis per virtual byte
    #[serde(rename = "fee_rate")]
    pub fee_rate: f64,
}

/// Transform internal FeeEstimate to API response format
pub fn transform_fee_estimate(estimate: FeeEstimate) -> FeeEstimateResponse {
    let estimates = estimate
        .estimates
        .into_iter()
        .map(|(block_num, target)| {
            let block_key = block_num.to_string();
            let probabilities = transform_block_target(target);
            (block_key, BlockTargetResponse { probabilities })
        })
        .collect();

    FeeEstimateResponse {
        mempool_update_time: format_timestamp(estimate.timestamp),
        estimates,
    }
}

/// Transform internal BlockTarget to API format
fn transform_block_target(target: BlockTarget) -> BTreeMap<String, ProbabilityResponse> {
    target
        .probabilities
        .into_iter()
        .map(|(prob, fee_rate)| {
            // Format probability with 2 decimal places (e.g., "0.95")
            let prob_key = format!("{:.2}", prob.0);

            // Format fee rate with 4 decimal places, matching Kotlin
            let formatted_fee_rate = format!("{:.4}", fee_rate)
                .parse::<f64>()
                .unwrap_or(fee_rate);

            (
                prob_key,
                ProbabilityResponse {
                    fee_rate: formatted_fee_rate,
                },
            )
        })
        .collect()
}

/// Format timestamp to ISO 8601 with milliseconds and UTC timezone
fn format_timestamp(timestamp: DateTime<Utc>) -> String {
    // Format: "2025-01-20T12:00:00.000Z"
    timestamp.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// Create an empty response when no estimates are available
#[allow(dead_code)]
pub fn empty_response(timestamp: DateTime<Utc>) -> FeeEstimateResponse {
    FeeEstimateResponse {
        mempool_update_time: format_timestamp(timestamp),
        estimates: BTreeMap::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin_augur::OrderedFloat;
    use std::collections::BTreeMap;

    #[test]
    fn test_transform_fee_estimate() {
        let mut probabilities = BTreeMap::new();
        probabilities.insert(OrderedFloat(0.05), 2.0916);
        probabilities.insert(OrderedFloat(0.50), 3.4846);
        probabilities.insert(OrderedFloat(0.95), 5.0531);

        let block_target = BlockTarget {
            blocks: 6,
            probabilities,
        };

        let mut estimates = BTreeMap::new();
        estimates.insert(6, block_target);

        let fee_estimate = FeeEstimate {
            estimates,
            timestamp: Utc::now(),
        };

        let response = transform_fee_estimate(fee_estimate);

        assert!(response.estimates.contains_key("6"));
        let target = &response.estimates["6"];
        assert!(target.probabilities.contains_key("0.05"));
        assert!(target.probabilities.contains_key("0.50"));
        assert!(target.probabilities.contains_key("0.95"));
    }

    #[test]
    fn test_format_timestamp() {
        let timestamp = DateTime::parse_from_rfc3339("2025-01-20T12:00:00.123Z")
            .unwrap()
            .with_timezone(&Utc);

        let formatted = format_timestamp(timestamp);
        assert_eq!(formatted, "2025-01-20T12:00:00.123Z");
    }

    #[test]
    fn test_probability_formatting() {
        let mut probabilities = BTreeMap::new();
        probabilities.insert(OrderedFloat(0.05123), 2.091678);
        probabilities.insert(OrderedFloat(0.95456), 5.053189);

        let block_target = BlockTarget {
            blocks: 3,
            probabilities,
        };

        let transformed = transform_block_target(block_target);

        // Check that probabilities are formatted with 2 decimal places
        assert!(transformed.contains_key("0.05"));
        assert!(transformed.contains_key("0.95"));

        // Check that fee rates are formatted with 4 decimal places
        assert_eq!(transformed["0.05"].fee_rate, 2.0917);
        assert_eq!(transformed["0.95"].fee_rate, 5.0532);
    }
}
