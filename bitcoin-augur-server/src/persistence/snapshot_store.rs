use bitcoin_augur::MempoolSnapshot;
use chrono::{DateTime, Local};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, error, info};

/// Persistence layer errors
#[derive(Error, Debug)]
pub enum PersistenceError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Invalid timestamp: {0}")]
    InvalidTimestamp(i64),
}

/// Manages persistent storage of mempool snapshots
pub struct SnapshotStore {
    data_dir: PathBuf,
}

impl SnapshotStore {
    /// Creates a new snapshot store with the specified data directory
    pub fn new(data_dir: impl AsRef<Path>) -> Result<Self, PersistenceError> {
        let data_dir = data_dir.as_ref().to_path_buf();

        // Ensure the data directory exists
        fs::create_dir_all(&data_dir)?;

        info!("Initialized snapshot store at: {}", data_dir.display());

        Ok(Self { data_dir })
    }

    /// Saves a mempool snapshot to disk
    pub fn save_snapshot(&self, snapshot: &MempoolSnapshot) -> Result<(), PersistenceError> {
        // Create directory structure: data/YYYY-MM-DD/
        let date_str = snapshot.timestamp.format("%Y-%m-%d").to_string();
        let date_dir = self.data_dir.join(&date_str);
        fs::create_dir_all(&date_dir)?;

        // Create filename: blockheight_timestamp.json
        let filename = format!(
            "{}_{}.json",
            snapshot.block_height,
            snapshot.timestamp.timestamp()
        );
        let file_path = date_dir.join(filename);

        // Serialize and save snapshot
        let json = serde_json::to_string_pretty(snapshot)?;
        fs::write(&file_path, json)?;

        debug!("Saved snapshot to: {}", file_path.display());

        Ok(())
    }

    /// Retrieves snapshots within a time range
    pub fn get_snapshots(
        &self,
        start: DateTime<Local>,
        end: DateTime<Local>,
    ) -> Result<Vec<MempoolSnapshot>, PersistenceError> {
        let mut snapshots = Vec::new();

        // Iterate through date directories
        let mut current_date = start.date_naive();
        let end_date = end.date_naive();

        while current_date <= end_date {
            let date_str = current_date.format("%Y-%m-%d").to_string();
            let date_dir = self.data_dir.join(&date_str);

            if date_dir.exists() && date_dir.is_dir() {
                // Read all JSON files in the directory
                for entry in fs::read_dir(&date_dir)? {
                    let entry = entry?;
                    let path = entry.path();

                    if path.extension().and_then(|s| s.to_str()) == Some("json") {
                        // Parse the filename to check if it's within our time range
                        if let Some(timestamp) = Self::extract_timestamp_from_filename(&path) {
                            let snapshot_time = DateTime::from_timestamp(timestamp, 0)
                                .ok_or(PersistenceError::InvalidTimestamp(timestamp))?
                                .with_timezone(&Local);

                            if snapshot_time >= start && snapshot_time <= end {
                                // Load and parse the snapshot
                                let content = fs::read_to_string(&path)?;
                                let snapshot: MempoolSnapshot = serde_json::from_str(&content)?;
                                snapshots.push(snapshot);
                            }
                        }
                    }
                }
            }

            // Move to next day
            current_date = current_date
                .succ_opt()
                .ok_or_else(|| PersistenceError::InvalidPath("Date overflow".to_string()))?;
        }

        // Sort snapshots by timestamp
        snapshots.sort_by_key(|s| s.timestamp);

        debug!(
            "Retrieved {} snapshots from {} to {}",
            snapshots.len(),
            start.format("%Y-%m-%d %H:%M:%S"),
            end.format("%Y-%m-%d %H:%M:%S")
        );

        Ok(snapshots)
    }

    /// Gets the most recent snapshot
    #[allow(dead_code)]
    pub fn get_latest_snapshot(&self) -> Result<Option<MempoolSnapshot>, PersistenceError> {
        let mut latest: Option<(i64, PathBuf)> = None;

        // Scan all date directories
        for entry in fs::read_dir(&self.data_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Scan JSON files in this directory
                for file_entry in fs::read_dir(&path)? {
                    let file_entry = file_entry?;
                    let file_path = file_entry.path();

                    if file_path.extension().and_then(|s| s.to_str()) == Some("json") {
                        if let Some(timestamp) = Self::extract_timestamp_from_filename(&file_path) {
                            if latest.is_none() || timestamp > latest.as_ref().unwrap().0 {
                                latest = Some((timestamp, file_path));
                            }
                        }
                    }
                }
            }
        }

        if let Some((_, path)) = latest {
            let content = fs::read_to_string(&path)?;
            let snapshot: MempoolSnapshot = serde_json::from_str(&content)?;
            Ok(Some(snapshot))
        } else {
            Ok(None)
        }
    }

    /// Gets snapshots from the last N hours
    pub fn get_recent_snapshots(
        &self,
        hours: i64,
    ) -> Result<Vec<MempoolSnapshot>, PersistenceError> {
        let end = Local::now();
        let start = end - chrono::Duration::hours(hours);
        self.get_snapshots(start, end)
    }

    /// Cleans up old snapshots older than the specified number of days
    pub fn cleanup_old_snapshots(&self, days_to_keep: i64) -> Result<usize, PersistenceError> {
        let cutoff_date = Local::now().date_naive() - chrono::Duration::days(days_to_keep);
        let mut deleted_count = 0;

        for entry in fs::read_dir(&self.data_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Parse directory name as date
                if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                    if let Ok(dir_date) = chrono::NaiveDate::parse_from_str(dir_name, "%Y-%m-%d") {
                        if dir_date < cutoff_date {
                            // Delete old directory
                            fs::remove_dir_all(&path)?;
                            deleted_count += 1;
                            info!("Deleted old snapshot directory: {}", dir_name);
                        }
                    }
                }
            }
        }

        Ok(deleted_count)
    }

    /// Extracts timestamp from snapshot filename
    fn extract_timestamp_from_filename(path: &Path) -> Option<i64> {
        let filename = path.file_stem()?.to_str()?;
        let parts: Vec<&str> = filename.split('_').collect();

        if parts.len() >= 2 {
            parts.last()?.parse().ok()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin_augur::MempoolTransaction;
    use chrono::{TimeZone, Utc};
    use pretty_assertions::assert_eq;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_snapshot(block_height: u32, timestamp: DateTime<Utc>) -> MempoolSnapshot {
        let transactions = vec![
            MempoolTransaction::new(1000, 2000),
            MempoolTransaction::new(500, 1500),
            MempoolTransaction::new(250, 1000),
        ];
        MempoolSnapshot::from_transactions(transactions, block_height, timestamp)
    }

    #[test]
    fn test_snapshot_store_creation() -> Result<(), PersistenceError> {
        let temp_dir = TempDir::new().unwrap();
        let _store = SnapshotStore::new(temp_dir.path())?;

        // Check that directory was created
        assert!(temp_dir.path().exists());

        Ok(())
    }

    #[test]
    fn test_save_and_retrieve_snapshot() -> Result<(), PersistenceError> {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path())?;

        // Create a test snapshot
        let transactions = vec![
            MempoolTransaction::new(400, 1000),
            MempoolTransaction::new(600, 1500),
        ];

        let snapshot = MempoolSnapshot::from_transactions(transactions, 850000, Utc::now());

        // Save the snapshot
        store.save_snapshot(&snapshot)?;

        // Retrieve snapshots from the last hour
        let retrieved = store.get_recent_snapshots(1)?;

        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].block_height, 850000);

        Ok(())
    }

    #[test]
    fn test_get_latest_snapshot() -> Result<(), PersistenceError> {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path())?;

        // Save multiple snapshots
        for i in 0..3 {
            let snapshot = MempoolSnapshot::from_transactions(
                vec![MempoolTransaction::new(400, 1000)],
                850000 + i,
                Utc::now() + chrono::Duration::seconds(i as i64),
            );
            store.save_snapshot(&snapshot)?;
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        // Get the latest
        let latest = store.get_latest_snapshot()?.unwrap();
        assert_eq!(latest.block_height, 850002);

        Ok(())
    }

    #[test]
    fn test_cleanup_old_snapshots() -> Result<(), PersistenceError> {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path())?;

        // Create an old snapshot (3 days ago)
        let old_time = Utc::now() - chrono::Duration::days(3);
        let old_snapshot =
            MempoolSnapshot::new(850000, old_time, std::collections::BTreeMap::new());

        // Save it with manual path to ensure old date
        let date_str = old_time.format("%Y-%m-%d").to_string();
        let date_dir = temp_dir.path().join(&date_str);
        fs::create_dir_all(&date_dir)?;

        let filename = format!("{}_{}.json", 850000, old_time.timestamp());
        let file_path = date_dir.join(filename);
        let json = serde_json::to_string_pretty(&old_snapshot)?;
        fs::write(&file_path, json)?;

        // Also create a recent snapshot
        let recent_snapshot = MempoolSnapshot::from_transactions(
            vec![MempoolTransaction::new(400, 1000)],
            850001,
            Utc::now(),
        );
        store.save_snapshot(&recent_snapshot)?;

        // Clean up snapshots older than 2 days
        let deleted = store.cleanup_old_snapshots(2)?;
        assert_eq!(deleted, 1);

        // Verify recent snapshot still exists
        let remaining = store.get_recent_snapshots(1)?;
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].block_height, 850001);

        Ok(())
    }

    #[test]
    fn test_directory_structure() -> Result<(), PersistenceError> {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path())?;

        let timestamp = Utc.with_ymd_and_hms(2024, 6, 15, 14, 30, 0).unwrap();
        let snapshot = create_test_snapshot(850000, timestamp);

        store.save_snapshot(&snapshot)?;

        // Check that the correct directory structure was created
        let expected_dir = temp_dir.path().join("2024-06-15");
        assert!(expected_dir.exists());
        assert!(expected_dir.is_dir());

        // Check that the file exists with correct naming
        let expected_file = expected_dir.join(format!("850000_{}.json", timestamp.timestamp()));
        assert!(expected_file.exists());

        Ok(())
    }

    #[test]
    fn test_get_snapshots_time_range() -> Result<(), PersistenceError> {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path())?;

        // Create snapshots at different times
        let base_time = Utc::now();
        for i in 0..5 {
            let timestamp = base_time - chrono::Duration::hours(i * 2);
            let snapshot = create_test_snapshot(850000 + i as u32, timestamp);
            store.save_snapshot(&snapshot)?;
        }

        // Query a specific time range (last 6 hours)
        let start = Local::now() - chrono::Duration::hours(6);
        let end = Local::now();
        let snapshots = store.get_snapshots(start, end)?;

        // Should get 3 snapshots (0, 2, 4 hours ago)
        assert_eq!(snapshots.len(), 3);

        // Verify they're sorted by timestamp
        for i in 0..snapshots.len() - 1 {
            assert!(snapshots[i].timestamp <= snapshots[i + 1].timestamp);
        }

        Ok(())
    }

    #[test]
    fn test_extract_timestamp_from_filename() {
        use std::path::Path;

        let path = Path::new("/data/2024-06-15/850000_1718458200.json");
        let timestamp = SnapshotStore::extract_timestamp_from_filename(path);
        assert_eq!(timestamp, Some(1718458200));

        let path = Path::new("/data/850000.json");
        let timestamp = SnapshotStore::extract_timestamp_from_filename(path);
        assert_eq!(timestamp, None);

        let path = Path::new("/data/invalid_filename.json");
        let timestamp = SnapshotStore::extract_timestamp_from_filename(path);
        assert_eq!(timestamp, None);
    }

    #[test]
    fn test_persistence_error_handling() -> Result<(), PersistenceError> {
        // Test invalid path
        let result = SnapshotStore::new("/nonexistent/readonly/path");
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_corrupted_json_handling() -> Result<(), PersistenceError> {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path())?;

        // Create a valid snapshot first
        let snapshot = create_test_snapshot(850000, Utc::now());
        store.save_snapshot(&snapshot)?;

        // Manually create a corrupted JSON file
        let date_str = Utc::now().format("%Y-%m-%d").to_string();
        let date_dir = temp_dir.path().join(&date_str);
        let corrupted_file = date_dir.join("850001_1234567890.json");
        fs::write(&corrupted_file, "{ invalid json }")?;

        // Try to retrieve snapshots - should skip the corrupted one
        let result = store.get_recent_snapshots(1);

        // Should succeed but only return the valid snapshot
        assert!(result.is_ok());
        let snapshots = result?;
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].block_height, 850000);

        Ok(())
    }

    #[test]
    fn test_empty_directory_handling() -> Result<(), PersistenceError> {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path())?;

        // Query empty store
        let snapshots = store.get_recent_snapshots(24)?;
        assert_eq!(snapshots.len(), 0);

        let latest = store.get_latest_snapshot()?;
        assert!(latest.is_none());

        Ok(())
    }

    #[test]
    fn test_large_snapshot_handling() -> Result<(), PersistenceError> {
        let temp_dir = TempDir::new().unwrap();
        let store = SnapshotStore::new(temp_dir.path())?;

        // Create a large snapshot with many transactions
        let mut transactions = Vec::new();
        for i in 0..10000 {
            transactions.push(MempoolTransaction::new(400 + (i % 1000), 1000 + (i % 5000)));
        }

        let large_snapshot = MempoolSnapshot::from_transactions(transactions, 850000, Utc::now());

        // Save and retrieve
        store.save_snapshot(&large_snapshot)?;
        let retrieved = store.get_recent_snapshots(1)?;

        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].block_height, 850000);
        assert_eq!(
            retrieved[0].bucketed_weights.len(),
            large_snapshot.bucketed_weights.len()
        );

        Ok(())
    }
}
