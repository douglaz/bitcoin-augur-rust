use super::*;
use crate::bitcoin::BitcoinRpcConfig;
use bitcoin_augur::{BlockTarget, MempoolTransaction, OrderedFloat};
use chrono::Utc;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// Helper function to create test snapshot
fn create_test_snapshot(block_height: u32) -> MempoolSnapshot {
    let transactions = vec![
        MempoolTransaction::new(1000, 2000),
        MempoolTransaction::new(500, 1500),
        MempoolTransaction::new(250, 1000),
    ];
    MempoolSnapshot::from_transactions(transactions, block_height, Utc::now())
}

// Helper function to create test fee estimate
#[allow(dead_code)]
fn create_test_fee_estimate() -> FeeEstimate {
    let mut estimates = BTreeMap::new();

    // Add some block targets with confidence levels
    for target in [3, 6, 12, 24, 144] {
        let mut confidence_map = BTreeMap::new();
        let base_fee = 50.0 / (target as f64).sqrt(); // Decreasing fee with target

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
async fn test_collector_initialization() {
    let temp_dir = TempDir::new().unwrap();

    let bitcoin_client = BitcoinRpcClient::new(BitcoinRpcConfig {
        url: "http://localhost:8332".to_string(),
        username: "test".to_string(),
        password: "test".to_string(),
    });

    let snapshot_store = SnapshotStore::new(temp_dir.path()).unwrap();
    let fee_estimator = FeeEstimator::new();

    let collector = MempoolCollector::new(bitcoin_client, snapshot_store, fee_estimator);

    // Check initial state
    assert!(collector.get_latest_estimate().await.is_none());
    assert!(collector.get_latest_snapshot().await.is_none());
}

#[tokio::test]
async fn test_update_fee_estimates_success() {
    let mock_server = MockServer::start().await;
    let temp_dir = TempDir::new().unwrap();

    let bitcoin_client = BitcoinRpcClient::new(BitcoinRpcConfig {
        url: mock_server.uri(),
        username: "test".to_string(),
        password: "pass".to_string(),
    });

    // Mock successful RPC response
    Mock::given(method("POST"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            {
                "result": {
                    "blocks": 850000,
                    "bestblockhash": "hash"
                },
                "error": null,
                "id": "blockchain-info"
            },
            {
                "result": {
                    "tx1": {
                        "weight": 1000,
                        "fees": {
                            "base": 0.00002000
                        }
                    },
                    "tx2": {
                        "weight": 800,
                        "fees": {
                            "base": 0.00001500
                        }
                    }
                },
                "error": null,
                "id": "mempool"
            }
        ])))
        .mount(&mock_server)
        .await;

    let snapshot_store = SnapshotStore::new(temp_dir.path()).unwrap();
    let fee_estimator = FeeEstimator::new();

    let collector = MempoolCollector::new(bitcoin_client, snapshot_store, fee_estimator);

    // Update estimates
    let result = collector.update_fee_estimates().await;
    assert!(result.is_ok());

    // Check that snapshot was saved
    let latest_snapshot = collector.get_latest_snapshot().await;
    assert!(latest_snapshot.is_some());

    let snapshot = latest_snapshot.unwrap();
    assert_eq!(snapshot.block_height, 850000);
}

#[tokio::test]
async fn test_update_fee_estimates_rpc_failure() {
    let mock_server = MockServer::start().await;
    let temp_dir = TempDir::new().unwrap();

    let bitcoin_client = BitcoinRpcClient::new(BitcoinRpcConfig {
        url: mock_server.uri(),
        username: "test".to_string(),
        password: "pass".to_string(),
    });

    // Mock RPC error response
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock_server)
        .await;

    let snapshot_store = SnapshotStore::new(temp_dir.path()).unwrap();
    let fee_estimator = FeeEstimator::new();

    let collector = MempoolCollector::new(bitcoin_client, snapshot_store, fee_estimator);

    // Update should fail
    let result = collector.update_fee_estimates().await;
    assert!(result.is_err());

    // No snapshot should be saved
    let latest_snapshot = collector.get_latest_snapshot().await;
    assert!(latest_snapshot.is_none());
}

#[tokio::test]
async fn test_initialize_from_store() {
    let temp_dir = TempDir::new().unwrap();

    let bitcoin_client = BitcoinRpcClient::new(BitcoinRpcConfig {
        url: "http://localhost:8332".to_string(),
        username: "test".to_string(),
        password: "test".to_string(),
    });

    let snapshot_store = SnapshotStore::new(temp_dir.path()).unwrap();

    // Pre-populate store with some snapshots
    for i in 0..5 {
        let snapshot = create_test_snapshot(850000 + i);
        snapshot_store.save_snapshot(&snapshot).unwrap();
    }

    let fee_estimator = FeeEstimator::new();
    let collector = MempoolCollector::new(bitcoin_client, snapshot_store, fee_estimator);

    // Initialize from store
    let result = collector.initialize_from_store().await;
    assert!(result.is_ok());

    // Should have latest snapshot
    let latest_snapshot = collector.get_latest_snapshot().await;
    assert!(latest_snapshot.is_some());
    assert_eq!(latest_snapshot.unwrap().block_height, 850004);
}

#[tokio::test]
async fn test_get_estimate_for_blocks() {
    let temp_dir = TempDir::new().unwrap();

    let bitcoin_client = BitcoinRpcClient::new(BitcoinRpcConfig {
        url: "http://localhost:8332".to_string(),
        username: "test".to_string(),
        password: "test".to_string(),
    });

    let snapshot_store = SnapshotStore::new(temp_dir.path()).unwrap();

    // Pre-populate store with snapshots
    for i in 0..10 {
        let snapshot = create_test_snapshot(850000 + i);
        snapshot_store.save_snapshot(&snapshot).unwrap();
    }

    let fee_estimator = FeeEstimator::new();
    let collector = MempoolCollector::new(bitcoin_client, snapshot_store, fee_estimator);

    // Get estimate for 6 blocks
    let result = collector.get_estimate_for_blocks(6.0).await;
    assert!(result.is_ok());

    let estimate = result.unwrap();
    assert!(!estimate.estimates.is_empty());
}

#[tokio::test]
async fn test_get_estimate_for_blocks_empty_store() {
    let temp_dir = TempDir::new().unwrap();

    let bitcoin_client = BitcoinRpcClient::new(BitcoinRpcConfig {
        url: "http://localhost:8332".to_string(),
        username: "test".to_string(),
        password: "test".to_string(),
    });

    let snapshot_store = SnapshotStore::new(temp_dir.path()).unwrap();
    let fee_estimator = FeeEstimator::new();
    let collector = MempoolCollector::new(bitcoin_client, snapshot_store, fee_estimator);

    // Get estimate with empty store
    let result = collector.get_estimate_for_blocks(6.0).await;
    assert!(result.is_ok());

    let estimate = result.unwrap();
    assert!(estimate.estimates.is_empty()); // Should return empty estimate
}

#[tokio::test]
async fn test_get_estimate_for_timestamp() {
    let temp_dir = TempDir::new().unwrap();

    let bitcoin_client = BitcoinRpcClient::new(BitcoinRpcConfig {
        url: "http://localhost:8332".to_string(),
        username: "test".to_string(),
        password: "test".to_string(),
    });

    let snapshot_store = SnapshotStore::new(temp_dir.path()).unwrap();

    // Create snapshots at specific times
    let base_time = Utc::now();
    for i in 0..5 {
        let timestamp = base_time - chrono::Duration::hours(i * 2);
        let transactions = vec![
            MempoolTransaction::new(1000, 2000),
            MempoolTransaction::new(500, 1500),
        ];
        let snapshot =
            MempoolSnapshot::from_transactions(transactions, 850000 + i as u32, timestamp);
        snapshot_store.save_snapshot(&snapshot).unwrap();
    }

    let fee_estimator = FeeEstimator::new();
    let collector = MempoolCollector::new(bitcoin_client, snapshot_store, fee_estimator);

    // Get estimate for current timestamp
    let result = collector
        .get_estimate_for_timestamp(base_time.timestamp())
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_get_estimate_for_invalid_timestamp() {
    let temp_dir = TempDir::new().unwrap();

    let bitcoin_client = BitcoinRpcClient::new(BitcoinRpcConfig {
        url: "http://localhost:8332".to_string(),
        username: "test".to_string(),
        password: "test".to_string(),
    });

    let snapshot_store = SnapshotStore::new(temp_dir.path()).unwrap();
    let fee_estimator = FeeEstimator::new();
    let collector = MempoolCollector::new(bitcoin_client, snapshot_store, fee_estimator);

    // Use an invalid timestamp (negative)
    let result = collector.get_estimate_for_timestamp(-1).await;
    assert!(result.is_err());

    // Use a timestamp far in the future (year 3000)
    let future_timestamp = 32503680000i64;
    let result = collector.get_estimate_for_timestamp(future_timestamp).await;
    assert!(result.is_ok());
    let estimate = result.unwrap();
    assert!(estimate.estimates.is_empty()); // No data available
}

#[tokio::test]
async fn test_cleanup_old_snapshots() {
    let temp_dir = TempDir::new().unwrap();

    let bitcoin_client = BitcoinRpcClient::new(BitcoinRpcConfig {
        url: "http://localhost:8332".to_string(),
        username: "test".to_string(),
        password: "test".to_string(),
    });

    let snapshot_store = SnapshotStore::new(temp_dir.path()).unwrap();

    // Create old snapshots
    for days_ago in 0..7 {
        let old_time = Utc::now() - chrono::Duration::days(days_ago);
        let snapshot = MempoolSnapshot::new(
            850000 + days_ago as u32,
            old_time,
            std::collections::BTreeMap::new(),
        );

        // Manually save with correct date directory
        let date_str = old_time.format("%Y-%m-%d").to_string();
        let date_dir = temp_dir.path().join(&date_str);
        std::fs::create_dir_all(&date_dir).unwrap();

        let filename = format!("{}_{}.json", snapshot.block_height, old_time.timestamp());
        let file_path = date_dir.join(filename);
        let json = serde_json::to_string_pretty(&snapshot).unwrap();
        std::fs::write(&file_path, json).unwrap();
    }

    let fee_estimator = FeeEstimator::new();
    let collector = MempoolCollector::new(bitcoin_client, snapshot_store, fee_estimator);

    // Cleanup snapshots older than 3 days
    let result = collector.cleanup_old_snapshots(3).await;
    assert!(result.is_ok());

    let deleted_count = result.unwrap();
    assert!(deleted_count >= 3); // Should delete at least 3 old directories
}

#[tokio::test]
async fn test_test_connection() {
    let mock_server = MockServer::start().await;

    let bitcoin_client = BitcoinRpcClient::new(BitcoinRpcConfig {
        url: mock_server.uri(),
        username: "test".to_string(),
        password: "pass".to_string(),
    });

    // Mock successful connection test
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "result": 850000,
            "error": null,
            "id": "test"
        })))
        .mount(&mock_server)
        .await;

    let temp_dir = TempDir::new().unwrap();
    let snapshot_store = SnapshotStore::new(temp_dir.path()).unwrap();
    let fee_estimator = FeeEstimator::new();

    let collector = MempoolCollector::new(bitcoin_client, snapshot_store, fee_estimator);

    let result = collector.test_connection().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_concurrent_access() {
    let temp_dir = TempDir::new().unwrap();

    let bitcoin_client = BitcoinRpcClient::new(BitcoinRpcConfig {
        url: "http://localhost:8332".to_string(),
        username: "test".to_string(),
        password: "test".to_string(),
    });

    let snapshot_store = SnapshotStore::new(temp_dir.path()).unwrap();
    let fee_estimator = FeeEstimator::new();
    let collector = Arc::new(MempoolCollector::new(
        bitcoin_client,
        snapshot_store,
        fee_estimator,
    ));

    // Spawn multiple concurrent tasks accessing the collector
    let mut handles = vec![];

    for i in 0..5 {
        let collector_clone = Arc::clone(&collector);
        let handle = tokio::spawn(async move {
            // Each task tries to get estimates
            let _ = collector_clone
                .get_estimate_for_blocks(6.0 + i as f64)
                .await;
            let _ = collector_clone.get_latest_estimate().await;
            let _ = collector_clone.get_latest_snapshot().await;
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test]
async fn test_persistence_error_propagation() {
    // Use an invalid path that will cause persistence errors
    let _bitcoin_client = BitcoinRpcClient::new(BitcoinRpcConfig {
        url: "http://localhost:8332".to_string(),
        username: "test".to_string(),
        password: "test".to_string(),
    });

    // This should fail to create the snapshot store
    let result = SnapshotStore::new("/nonexistent/readonly/path");
    assert!(result.is_err());
}
