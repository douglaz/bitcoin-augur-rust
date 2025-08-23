use axum::http::StatusCode;
use bitcoin_augur::{FeeEstimator, MempoolSnapshot};
use bitcoin_augur_server::bitcoin::{BitcoinRpcClient, BitcoinRpcConfig};
use bitcoin_augur_server::persistence::SnapshotStore;
use bitcoin_augur_server::server::create_app;
use bitcoin_augur_server::service::MempoolCollector;
use chrono::Utc;
use std::collections::BTreeMap;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

/// Create test app with mock collector
async fn create_test_app() -> anyhow::Result<axum::Router> {
    let temp_dir = TempDir::new()?;
    let config = BitcoinRpcConfig {
        url: "http://localhost:8332".to_string(),
        username: "test".to_string(),
        password: "test".to_string(),
    };

    let bitcoin_client = BitcoinRpcClient::new(config);
    let snapshot_store = SnapshotStore::new(temp_dir.path())?;
    let fee_estimator = FeeEstimator::new();

    // Save test snapshots to persistence
    let snapshots = create_test_snapshots();
    for snapshot in &snapshots {
        snapshot_store.save_snapshot(snapshot)?;
    }

    let collector = Arc::new(MempoolCollector::new(
        bitcoin_client,
        snapshot_store,
        fee_estimator,
    ));

    // Initialize the collector with estimates from the saved snapshots
    collector.initialize_from_store().await?;

    Ok(create_app(collector))
}

/// Create test snapshots with realistic data
fn create_test_snapshots() -> Vec<MempoolSnapshot> {
    let mut snapshots = vec![];
    let base_time = Utc::now();

    // Create snapshots with varying fee distributions
    for i in 0..5 {
        let mut bucketed_weights = BTreeMap::new();

        // Add weights to different fee buckets
        // Bucket index is ln(fee_rate) * 100
        bucketed_weights.insert(100, 1000 * (i + 1)); // ~2.7 sat/vB
        bucketed_weights.insert(200, 2000 * (i + 1)); // ~7.4 sat/vB
        bucketed_weights.insert(300, 3000 * (i + 1)); // ~20 sat/vB
        bucketed_weights.insert(400, 1500 * (i + 1)); // ~55 sat/vB
        bucketed_weights.insert(500, 500 * (i + 1)); // ~148 sat/vB

        let snapshot = MempoolSnapshot::new(
            911275 + i as u32,
            base_time - chrono::Duration::minutes(i as i64 * 10),
            bucketed_weights,
        );
        snapshots.push(snapshot);
    }

    snapshots
}

#[tokio::test]
async fn test_health_endpoint() -> anyhow::Result<()> {
    let app = create_test_app().await?;

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/health")
                .body(axum::body::Body::empty())?,
        )
        .await?;

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), 1024).await?;
    let body_str = String::from_utf8(body.to_vec())?;
    assert_eq!(body_str, "OK");

    Ok(())
}

#[tokio::test]
async fn test_fees_endpoint() -> anyhow::Result<()> {
    let app = create_test_app().await?;

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/fees")
                .body(axum::body::Body::empty())?,
        )
        .await?;

    // Should return OK with fee estimates
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), 10240).await?;
    let fee_response: serde_json::Value = serde_json::from_slice(&body)?;

    // Verify response structure
    assert!(fee_response["mempool_update_time"].is_string());
    assert!(fee_response["estimates"].is_object());

    let estimates = fee_response["estimates"].as_object().unwrap();

    // Check that we have expected block targets
    for target in ["3", "6", "12", "24"] {
        assert!(estimates.contains_key(target));

        let block_target = &estimates[target];
        assert!(block_target["probabilities"].is_object());

        let probabilities = block_target["probabilities"].as_object().unwrap();

        // Check standard probability exists
        assert!(probabilities.contains_key("0.50"));
        assert!(probabilities["0.50"]["fee_rate"].as_f64().unwrap() > 0.0);
    }

    Ok(())
}

#[tokio::test]
async fn test_fees_target_endpoint() -> anyhow::Result<()> {
    let app = create_test_app().await?;

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/fees/target/6")
                .body(axum::body::Body::empty())?,
        )
        .await?;

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), 10240).await?;
    let fee_response: serde_json::Value = serde_json::from_slice(&body)?;

    // Should only have the requested target
    let estimates = fee_response["estimates"].as_object().unwrap();
    assert_eq!(estimates.len(), 1);
    assert!(estimates.contains_key("6"));

    let block_target = &estimates["6"];
    let probabilities = block_target["probabilities"].as_object().unwrap();

    // Check all probabilities are present
    for prob in ["0.05", "0.20", "0.50", "0.80", "0.95"] {
        assert!(probabilities.contains_key(prob));
        assert!(probabilities[prob]["fee_rate"].as_f64().unwrap() >= 1.0);
    }

    Ok(())
}

#[tokio::test]
async fn test_invalid_target() -> anyhow::Result<()> {
    let app = create_test_app().await?;

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/fees/target/1001") // Over 1000 limit
                .body(axum::body::Body::empty())?,
        )
        .await?;

    // Should return 400 for invalid target
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn test_historical_endpoint() -> anyhow::Result<()> {
    let app = create_test_app().await?;

    // Test with a specific timestamp
    let timestamp = Utc::now().timestamp();
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/historical_fee?timestamp={}", timestamp))
                .body(axum::body::Body::empty())?,
        )
        .await?;

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), 10240).await?;
    let fee_response: serde_json::Value = serde_json::from_slice(&body)?;

    // Verify response structure
    assert!(fee_response["mempool_update_time"].is_string());
    assert!(fee_response["estimates"].is_object());

    Ok(())
}

#[tokio::test]
async fn test_historical_missing_timestamp() -> anyhow::Result<()> {
    let app = create_test_app().await?;

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/historical_fee")
                .body(axum::body::Body::empty())?,
        )
        .await?;

    // Should return 400 for missing timestamp
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn test_concurrent_requests() -> anyhow::Result<()> {
    let app = create_test_app().await?;

    // Send multiple concurrent requests
    let mut handles = vec![];

    for _ in 0..5 {
        let app_clone = app.clone();
        let handle = tokio::spawn(async move {
            let response = app_clone
                .oneshot(
                    axum::http::Request::builder()
                        .uri("/fees")
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            response.status()
        });
        handles.push(handle);
    }

    // All requests should succeed
    for handle in handles {
        let status = handle.await?;
        assert_eq!(status, StatusCode::OK);
    }

    Ok(())
}
