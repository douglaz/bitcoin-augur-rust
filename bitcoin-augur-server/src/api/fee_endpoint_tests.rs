use super::*;
use crate::bitcoin::{BitcoinRpcClient, BitcoinRpcConfig};
use crate::persistence::SnapshotStore;
use bitcoin_augur::{
    BlockTarget, FeeEstimate, FeeEstimator, MempoolSnapshot, MempoolTransaction, OrderedFloat,
};
use chrono::Utc;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use tempfile::TempDir;

// Helper to create test collector with pre-populated data
async fn create_populated_collector() -> Arc<MempoolCollector> {
    let temp_dir = TempDir::new().unwrap();
    let config = BitcoinRpcConfig {
        url: "http://localhost:8332".to_string(),
        username: "test".to_string(),
        password: "test".to_string(),
    };

    let bitcoin_client = BitcoinRpcClient::new(config);
    let snapshot_store = SnapshotStore::new(temp_dir.path()).unwrap();

    // Pre-populate store with snapshots
    for i in 0..10 {
        let transactions = vec![
            MempoolTransaction::new(1000 + i * 100, 2000 + i * 50),
            MempoolTransaction::new(500 + i * 50, 1500 + i * 25),
        ];
        let snapshot = MempoolSnapshot::from_transactions(
            transactions,
            850000 + i as u32,
            Utc::now() - chrono::Duration::hours(10 - i as i64),
        );
        snapshot_store.save_snapshot(&snapshot).unwrap();
    }

    let fee_estimator = FeeEstimator::new();
    let collector = Arc::new(MempoolCollector::new(
        bitcoin_client,
        snapshot_store,
        fee_estimator,
    ));

    // Initialize from store
    collector.initialize_from_store().await.unwrap();

    collector
}

// Helper to create test fee estimate
fn create_test_estimate() -> FeeEstimate {
    let mut estimates = BTreeMap::new();

    for target in [3, 6, 12, 24, 144] {
        let mut confidence_map = BTreeMap::new();
        let base_fee = 50.0 / (target as f64).sqrt();

        confidence_map.insert(OrderedFloat(0.5), base_fee * 0.8);
        confidence_map.insert(OrderedFloat(0.8), base_fee);
        confidence_map.insert(OrderedFloat(0.95), base_fee * 1.2);

        let block_target = BlockTarget::new(target, confidence_map);
        estimates.insert(target, block_target);
    }

    FeeEstimate {
        timestamp: Utc::now(),
        estimates,
    }
}

#[tokio::test]
async fn test_get_fees_with_data() {
    let collector = create_populated_collector().await;
    let response = get_fees(State(collector)).await;

    assert_eq!(response.status(), StatusCode::OK);

    // Parse response body
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    // Should contain JSON with estimates
    assert!(body_str.contains("\"estimates\""));
    assert!(body_str.contains("\"timestamp\""));
}

#[tokio::test]
async fn test_get_fees_empty_collector() {
    let temp_dir = TempDir::new().unwrap();
    let config = BitcoinRpcConfig {
        url: "http://localhost:8332".to_string(),
        username: "test".to_string(),
        password: "test".to_string(),
    };

    let bitcoin_client = BitcoinRpcClient::new(config);
    let snapshot_store = SnapshotStore::new(temp_dir.path()).unwrap();
    let fee_estimator = FeeEstimator::new();

    let collector = Arc::new(MempoolCollector::new(
        bitcoin_client,
        snapshot_store,
        fee_estimator,
    ));

    let response = get_fees(State(collector)).await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert_eq!(body_str, "No fee estimates available yet");
}

#[tokio::test]
async fn test_get_fee_for_target_valid() {
    let collector = create_populated_collector().await;

    // Test various valid block targets
    for target in [1.0, 6.0, 12.5, 24.0, 144.0, 500.0] {
        let response = get_fee_for_target(Path(target), State(collector.clone())).await;
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();

        assert!(body_str.contains("\"estimates\""));
        assert!(body_str.contains("\"timestamp\""));
    }
}

#[tokio::test]
async fn test_get_fee_for_target_edge_cases() {
    let collector = create_populated_collector().await;

    // Test zero blocks
    let response = get_fee_for_target(Path(0.0), State(collector.clone())).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Test exactly at boundary
    let response = get_fee_for_target(Path(1000.0), State(collector.clone())).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Test just over boundary
    let response = get_fee_for_target(Path(1000.1), State(collector.clone())).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Test infinity
    let response = get_fee_for_target(Path(f64::INFINITY), State(collector.clone())).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Test negative infinity
    let response = get_fee_for_target(Path(f64::NEG_INFINITY), State(collector.clone())).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_get_fee_for_target_fractional_blocks() {
    let collector = create_populated_collector().await;

    // Test fractional block values
    for target in [0.5, 1.5, 6.75, 12.25] {
        let response = get_fee_for_target(Path(target), State(collector.clone())).await;
        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[tokio::test]
async fn test_get_fee_for_target_no_data() {
    let temp_dir = TempDir::new().unwrap();
    let config = BitcoinRpcConfig {
        url: "http://localhost:8332".to_string(),
        username: "test".to_string(),
        password: "test".to_string(),
    };

    let bitcoin_client = BitcoinRpcClient::new(config);
    let snapshot_store = SnapshotStore::new(temp_dir.path()).unwrap();
    let fee_estimator = FeeEstimator::new();

    let collector = Arc::new(MempoolCollector::new(
        bitcoin_client,
        snapshot_store,
        fee_estimator,
    ));

    let response = get_fee_for_target(Path(6.0), State(collector)).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn test_transform_fee_estimate() {
    let fee_estimate = create_test_estimate();
    let response = transform_fee_estimate(fee_estimate.clone());

    assert_eq!(response.estimates.len(), fee_estimate.estimates.len());

    // Check that transformation preserves data correctly
    for (target_blocks, target) in &fee_estimate.estimates {
        let target_key = target_blocks.to_string();

        assert!(response.estimates.contains_key(&target_key));

        let response_target = &response.estimates[&target_key];
        for (confidence, fee_rate) in &target.probabilities {
            let conf_key = format!("{:.2}", confidence.0);
            assert!(response_target.probabilities.contains_key(&conf_key));
            assert_eq!(response_target.probabilities[&conf_key].fee_rate, *fee_rate);
        }
    }
}

#[tokio::test]
async fn test_concurrent_requests() {
    let collector = create_populated_collector().await;

    // Spawn multiple concurrent requests
    let mut handles = vec![];

    for i in 0..10 {
        let collector_clone = collector.clone();
        let handle = tokio::spawn(async move {
            if i % 2 == 0 {
                get_fees(State(collector_clone)).await
            } else {
                get_fee_for_target(Path((i + 1) as f64), State(collector_clone)).await
            }
        });
        handles.push(handle);
    }

    // All requests should complete successfully
    for handle in handles {
        let response = handle.await.unwrap();
        assert!(
            response.status().is_success() || response.status() == StatusCode::SERVICE_UNAVAILABLE
        );
    }
}

#[tokio::test]
async fn test_response_format() {
    let collector = create_populated_collector().await;
    let response = get_fees(State(collector)).await;

    assert_eq!(response.status(), StatusCode::OK);

    // Parse and validate JSON response structure
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // Check top-level fields
    assert!(json.get("timestamp").is_some());
    assert!(json.get("estimates").is_some());

    // Check estimates structure
    let estimates = json.get("estimates").unwrap().as_object().unwrap();
    assert!(!estimates.is_empty());

    // Check that each estimate has confidence levels
    for (_target, confidences) in estimates {
        assert!(confidences.is_object());
        let conf_obj = confidences.as_object().unwrap();
        assert!(!conf_obj.is_empty());

        // Each confidence level should map to a number
        for (_conf, fee_rate) in conf_obj {
            assert!(fee_rate.is_number());
        }
    }
}

#[tokio::test]
async fn test_error_response_format() {
    let collector = create_populated_collector().await;

    // Test error response for invalid input
    let response = get_fee_for_target(Path(-10.0), State(collector)).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert_eq!(body_str, "Invalid or missing number of blocks");
}
