use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::service::MempoolCollector;
use super::models::{transform_fee_estimate, empty_response};

/// Query parameters for historical fee endpoint
#[derive(Debug, Deserialize)]
pub struct HistoricalQuery {
    /// Unix timestamp in seconds
    timestamp: i64,
}

/// GET /historical_fee?timestamp={unix_ts} - Returns historical fee estimates
pub async fn get_historical_fee(
    Query(params): Query<HistoricalQuery>,
    State(collector): State<Arc<MempoolCollector>>,
) -> Response {
    info!("Received request for historical fee estimates at timestamp {}", params.timestamp);
    
    // Validate timestamp (must be reasonable - not in future, not too far in past)
    let now = chrono::Utc::now().timestamp();
    if params.timestamp > now {
        warn!("Timestamp {} is in the future", params.timestamp);
        return (
            StatusCode::BAD_REQUEST,
            "Timestamp cannot be in the future"
        ).into_response();
    }
    
    // Don't allow timestamps more than 1 year in the past
    let one_year_ago = now - (365 * 24 * 60 * 60);
    if params.timestamp < one_year_ago {
        warn!("Timestamp {} is too far in the past", params.timestamp);
        return (
            StatusCode::BAD_REQUEST,
            "Timestamp is too far in the past (max 1 year)"
        ).into_response();
    }
    
    // Get historical estimate
    match collector.get_estimate_for_timestamp(params.timestamp).await {
        Ok(estimate) => {
            if estimate.estimates.is_empty() {
                debug!("No historical data available for timestamp {}", params.timestamp);
                (
                    StatusCode::NOT_FOUND,
                    "No historical data available for the requested timestamp"
                ).into_response()
            } else {
                let response = transform_fee_estimate(estimate);
                debug!("Returning historical fee estimates with {} targets", response.estimates.len());
                Json(response).into_response()
            }
        }
        Err(err) => {
            warn!("Failed to get historical fee estimates: {}", err);
            
            // Check error type for appropriate response
            if err.to_string().contains("InvalidTimestamp") {
                (
                    StatusCode::BAD_REQUEST,
                    "Invalid timestamp format"
                ).into_response()
            } else if err.to_string().contains("Insufficient") {
                (
                    StatusCode::NOT_FOUND,
                    "No historical data available for the requested timestamp"
                ).into_response()
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to retrieve historical fee estimates"
                ).into_response()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitcoin::{BitcoinRpcClient, BitcoinRpcConfig};
    use crate::persistence::SnapshotStore;
    use bitcoin_augur::FeeEstimator;
    use tempfile::TempDir;
    
    async fn create_test_collector() -> Arc<MempoolCollector> {
        let temp_dir = TempDir::new().unwrap();
        let config = BitcoinRpcConfig {
            url: "http://localhost:8332".to_string(),
            username: "test".to_string(),
            password: "test".to_string(),
        };
        
        let bitcoin_client = BitcoinRpcClient::new(config);
        let snapshot_store = SnapshotStore::new(temp_dir.path()).unwrap();
        let fee_estimator = FeeEstimator::new();
        
        Arc::new(MempoolCollector::new(
            bitcoin_client,
            snapshot_store,
            fee_estimator,
        ))
    }
    
    #[tokio::test]
    async fn test_historical_fee_future_timestamp() {
        let collector = create_test_collector().await;
        let future_timestamp = chrono::Utc::now().timestamp() + 3600; // 1 hour in future
        
        let response = get_historical_fee(
            Query(HistoricalQuery { timestamp: future_timestamp }),
            State(collector)
        ).await;
        
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    
    #[tokio::test]
    async fn test_historical_fee_old_timestamp() {
        let collector = create_test_collector().await;
        let old_timestamp = chrono::Utc::now().timestamp() - (400 * 24 * 60 * 60); // 400 days ago
        
        let response = get_historical_fee(
            Query(HistoricalQuery { timestamp: old_timestamp }),
            State(collector)
        ).await;
        
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    
    #[tokio::test]
    async fn test_historical_fee_no_data() {
        let collector = create_test_collector().await;
        let recent_timestamp = chrono::Utc::now().timestamp() - 3600; // 1 hour ago
        
        let response = get_historical_fee(
            Query(HistoricalQuery { timestamp: recent_timestamp }),
            State(collector)
        ).await;
        
        // Should return 404 when no data exists for timestamp
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}