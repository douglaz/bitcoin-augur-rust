use bitcoin_augur::{FeeEstimate, FeeEstimator, MempoolSnapshot, MempoolTransaction};
use chrono::{DateTime, Local, Utc};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};

use crate::bitcoin::{BitcoinRpcClient, RpcError};
use crate::persistence::{PersistenceError, SnapshotStore};

/// Mempool collector errors
#[derive(Error, Debug)]
pub enum CollectorError {
    #[error("RPC error: {0}")]
    RpcError(#[from] RpcError),
    
    #[error("Persistence error: {0}")]
    PersistenceError(#[from] PersistenceError),
    
    #[error("Estimation error: {0}")]
    EstimationError(#[from] bitcoin_augur::AugurError),
    
    #[error("Service is shutting down")]
    Shutdown,
}

/// Service that periodically collects mempool data and calculates fee estimates
pub struct MempoolCollector {
    bitcoin_client: Arc<BitcoinRpcClient>,
    snapshot_store: Arc<SnapshotStore>,
    fee_estimator: Arc<FeeEstimator>,
    latest_estimate: Arc<RwLock<Option<FeeEstimate>>>,
    latest_snapshot: Arc<RwLock<Option<MempoolSnapshot>>>,
}

impl MempoolCollector {
    /// Creates a new mempool collector
    pub fn new(
        bitcoin_client: BitcoinRpcClient,
        snapshot_store: SnapshotStore,
        fee_estimator: FeeEstimator,
    ) -> Self {
        Self {
            bitcoin_client: Arc::new(bitcoin_client),
            snapshot_store: Arc::new(snapshot_store),
            fee_estimator: Arc::new(fee_estimator),
            latest_estimate: Arc::new(RwLock::new(None)),
            latest_snapshot: Arc::new(RwLock::new(None)),
        }
    }
    
    /// Starts the collection service with the specified interval
    pub async fn start(&self, interval_ms: u64) -> Result<(), CollectorError> {
        let mut interval = interval(Duration::from_millis(interval_ms));
        
        info!("Starting mempool collector with {}ms interval", interval_ms);
        
        // Perform initial collection immediately
        if let Err(e) = self.update_fee_estimates().await {
            warn!("Initial fee estimate update failed: {}", e);
        }
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.update_fee_estimates().await {
                error!("Failed to update fee estimates: {}", e);
                // Continue running despite errors
            }
        }
    }
    
    /// Updates fee estimates by collecting fresh mempool data
    async fn update_fee_estimates(&self) -> Result<(), CollectorError> {
        debug!("Updating fee estimates");
        
        // Fetch current mempool data from Bitcoin Core
        let (height, transactions) = self.bitcoin_client
            .get_height_and_mempool()
            .await?;
        
        // Create snapshot
        let snapshot = MempoolSnapshot::from_transactions(
            transactions,
            height,
            Utc::now(),
        );
        
        // Save snapshot to disk
        self.snapshot_store.save_snapshot(&snapshot)?;
        
        // Update latest snapshot
        {
            let mut latest = self.latest_snapshot.write().await;
            *latest = Some(snapshot.clone());
        }
        
        // Get last 24 hours of snapshots for estimation
        let snapshots = self.snapshot_store.get_recent_snapshots(24)?;
        
        if !snapshots.is_empty() {
            // Calculate new fee estimates
            match self.fee_estimator.calculate_estimates(&snapshots, None) {
                Ok(estimate) => {
                    info!("Successfully calculated fee estimates with {} block targets", 
                          estimate.estimates.len());
                    
                    // Update latest estimate
                    let mut latest = self.latest_estimate.write().await;
                    *latest = Some(estimate);
                }
                Err(e) => {
                    warn!("Failed to calculate fee estimates: {}", e);
                }
            }
        } else {
            warn!("No historical snapshots available for fee estimation");
        }
        
        Ok(())
    }
    
    /// Gets the latest fee estimate
    pub async fn get_latest_estimate(&self) -> Option<FeeEstimate> {
        self.latest_estimate.read().await.clone()
    }
    
    /// Gets the latest mempool snapshot
    pub async fn get_latest_snapshot(&self) -> Option<MempoolSnapshot> {
        self.latest_snapshot.read().await.clone()
    }
    
    /// Calculates fee estimates for a specific block target
    pub async fn get_estimate_for_blocks(&self, num_blocks: f64) -> Result<FeeEstimate, CollectorError> {
        // Get recent snapshots
        let snapshots = self.snapshot_store.get_recent_snapshots(24)?;
        
        if snapshots.is_empty() {
            return Ok(FeeEstimate::empty(Utc::now()));
        }
        
        // Calculate estimates for specific target
        let estimate = self.fee_estimator.calculate_estimates(&snapshots, Some(num_blocks))?;
        Ok(estimate)
    }
    
    /// Gets fee estimate for a historical timestamp
    pub async fn get_estimate_for_timestamp(
        &self,
        timestamp: i64,
    ) -> Result<FeeEstimate, CollectorError> {
        let datetime = DateTime::from_timestamp(timestamp, 0)
            .ok_or(PersistenceError::InvalidTimestamp(timestamp))?
            .with_timezone(&Local);
        
        // Get snapshots from 24 hours before the target time
        let start = datetime - chrono::Duration::days(1);
        let snapshots = self.snapshot_store.get_snapshots(start, datetime)?;
        
        if snapshots.is_empty() {
            return Ok(FeeEstimate::empty(datetime.with_timezone(&Utc)));
        }
        
        let estimate = self.fee_estimator.calculate_estimates(&snapshots, None)?;
        Ok(estimate)
    }
    
    /// Performs cleanup of old snapshots
    pub async fn cleanup_old_snapshots(&self, days_to_keep: i64) -> Result<usize, CollectorError> {
        info!("Cleaning up snapshots older than {} days", days_to_keep);
        let deleted = self.snapshot_store.cleanup_old_snapshots(days_to_keep)?;
        info!("Deleted {} old snapshot directories", deleted);
        Ok(deleted)
    }
    
    /// Tests the Bitcoin RPC connection
    pub async fn test_connection(&self) -> Result<(), CollectorError> {
        self.bitcoin_client.test_connection().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitcoin::BitcoinRpcConfig;
    use tempfile::TempDir;
    
    fn create_test_config() -> BitcoinRpcConfig {
        BitcoinRpcConfig {
            url: "http://localhost:8332".to_string(),
            username: "test".to_string(),
            password: "test".to_string(),
        }
    }
    
    #[tokio::test]
    async fn test_collector_creation() {
        let temp_dir = TempDir::new().unwrap();
        
        let bitcoin_client = BitcoinRpcClient::new(create_test_config());
        let snapshot_store = SnapshotStore::new(temp_dir.path()).unwrap();
        let fee_estimator = FeeEstimator::new();
        
        let collector = MempoolCollector::new(
            bitcoin_client,
            snapshot_store,
            fee_estimator,
        );
        
        // Check initial state
        assert!(collector.get_latest_estimate().await.is_none());
        assert!(collector.get_latest_snapshot().await.is_none());
    }
}