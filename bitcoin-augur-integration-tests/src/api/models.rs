use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeEstimateResponse {
    #[serde(rename = "mempool_update_time")]
    pub mempool_update_time: Option<DateTime<Utc>>,

    #[serde(default)]
    pub estimates: HashMap<String, BlockTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockTarget {
    #[serde(default)]
    pub probabilities: HashMap<String, Probability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Probability {
    #[serde(rename = "fee_rate")]
    pub fee_rate: f64,
}

// Alternative response format for historical endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalFeeResponse {
    pub timestamp: DateTime<Utc>,
    pub estimates: FeeEstimateResponse,
}
