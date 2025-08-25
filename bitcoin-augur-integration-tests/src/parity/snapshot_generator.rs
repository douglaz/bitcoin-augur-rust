use anyhow::Result;
use bitcoin_augur::{MempoolSnapshot, MempoolTransaction};
use chrono::{Duration, Utc};
use std::fs;
use std::path::Path;
use tracing::{debug, info};

use super::test_data::{TestDataGenerator, TestSnapshot};

/// Pre-populates a data directory with mempool snapshots for testing
pub fn generate_and_save_snapshots(data_dir: &Path, hours: i64) -> Result<()> {
    info!("Generating {hours} hours of snapshot data");

    // Create data directory if it doesn't exist
    fs::create_dir_all(data_dir)?;

    // Generate snapshots - one every 10 minutes for the specified hours
    let snapshots_count = (hours * 6) as usize; // 6 snapshots per hour
    let start_time = Utc::now() - Duration::hours(hours);

    let test_snapshots = TestDataGenerator::create_snapshot_sequence(
        snapshots_count,
        1, // 1 snapshot per "block"
        start_time,
        Some(Duration::minutes(10)), // 10 minutes between snapshots
    );

    info!("Generated {} test snapshots", test_snapshots.len());

    // Convert and save each snapshot
    for (i, test_snapshot) in test_snapshots.into_iter().enumerate() {
        let mempool_snapshot = convert_to_mempool_snapshot(test_snapshot);
        save_snapshot_rust(data_dir, &mempool_snapshot)?;

        if i % 10 == 0 {
            debug!("Saved snapshot {}/{}", i + 1, snapshots_count);
        }
    }

    info!("Successfully saved all snapshots to {}", data_dir.display());
    Ok(())
}

/// Converts a TestSnapshot to a proper MempoolSnapshot with bucketed weights
fn convert_to_mempool_snapshot(test_snapshot: TestSnapshot) -> MempoolSnapshot {
    // Convert TestTransactions to MempoolTransactions
    let transactions: Vec<MempoolTransaction> = test_snapshot
        .transactions
        .into_iter()
        .map(|tx| MempoolTransaction::new(tx.weight, tx.fee))
        .collect();

    // Use the existing from_transactions method which handles bucketing
    MempoolSnapshot::from_transactions(
        transactions,
        test_snapshot.block_height,
        test_snapshot.timestamp,
    )
}

/// Saves a mempool snapshot to disk for Rust server (snake_case)
fn save_snapshot_rust(data_dir: &Path, snapshot: &MempoolSnapshot) -> Result<()> {
    // Create directory structure: data/YYYY-MM-DD/
    let date_str = snapshot.timestamp.format("%Y-%m-%d").to_string();
    let date_dir = data_dir.join(&date_str);
    fs::create_dir_all(&date_dir)?;

    // Create filename: blockheight_timestamp.json
    let filename = format!(
        "{}_{}.json",
        snapshot.block_height,
        snapshot.timestamp.timestamp()
    );
    let file_path = date_dir.join(filename);

    // Serialize and save snapshot (Rust uses snake_case)
    let json = serde_json::to_string_pretty(snapshot)?;
    fs::write(&file_path, json)?;

    Ok(())
}

/// Saves a mempool snapshot to disk for Kotlin server (camelCase)
fn save_snapshot_kotlin(data_dir: &Path, snapshot: &MempoolSnapshot) -> Result<()> {
    // Create directory structure: data/YYYY-MM-DD/
    let date_str = snapshot.timestamp.format("%Y-%m-%d").to_string();
    let date_dir = data_dir.join(&date_str);
    fs::create_dir_all(&date_dir)?;

    // Create filename: blockheight_timestamp.json
    let filename = format!(
        "{}_{}.json",
        snapshot.block_height,
        snapshot.timestamp.timestamp()
    );
    let file_path = date_dir.join(filename);

    // For Kotlin compatibility, we need to use camelCase field names
    let json_value = serde_json::json!({
        "blockHeight": snapshot.block_height,
        "timestamp": snapshot.timestamp.to_rfc3339(),
        "bucketedWeights": snapshot.bucketed_weights
    });

    let json = serde_json::to_string_pretty(&json_value)?;
    fs::write(&file_path, json)?;

    Ok(())
}

/// Pre-populates data directories for both Rust and Kotlin servers
pub fn setup_test_data(rust_data_dir: &Path, kotlin_data_dir: &Path) -> Result<()> {
    info!("Setting up test data for parity tests");

    // Generate snapshots (6 hours of data for faster testing)
    let hours = 6;
    let snapshots_count = (hours * 6) as usize;
    let start_time = Utc::now() - Duration::hours(hours);

    let test_snapshots = TestDataGenerator::create_snapshot_sequence(
        snapshots_count,
        1, // 1 snapshot per "block"
        start_time,
        Some(Duration::minutes(10)),
    );

    info!("Generated {} test snapshots", test_snapshots.len());

    // Save snapshots for both servers with their respective formats
    for test_snapshot in test_snapshots {
        let mempool_snapshot = convert_to_mempool_snapshot(test_snapshot);

        // Save for Rust server (snake_case)
        save_snapshot_rust(rust_data_dir, &mempool_snapshot)?;

        // Save for Kotlin server (camelCase)
        save_snapshot_kotlin(kotlin_data_dir, &mempool_snapshot)?;
    }

    info!("Test data setup complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_snapshot_generation() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let data_dir = temp_dir.path();

        // Generate 1 hour of data for testing
        generate_and_save_snapshots(data_dir, 1)?;

        // Verify files were created
        let mut snapshot_count = 0;
        for entry in fs::read_dir(data_dir)? {
            let entry = entry?;
            if entry.path().is_dir() {
                for file_entry in fs::read_dir(entry.path())? {
                    let file_entry = file_entry?;
                    if file_entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
                        snapshot_count += 1;
                    }
                }
            }
        }

        // Should have 6 snapshots for 1 hour (one every 10 minutes)
        assert_eq!(snapshot_count, 6);

        Ok(())
    }

    #[test]
    fn test_convert_snapshot() -> Result<()> {
        use super::super::test_data::TestTransaction;

        let test_snapshot = TestSnapshot {
            block_height: 850000,
            timestamp: Utc::now(),
            transactions: vec![
                TestTransaction {
                    weight: 1000,
                    fee: 10000,
                    fee_rate: 10.0,
                },
                TestTransaction {
                    weight: 2000,
                    fee: 40000,
                    fee_rate: 20.0,
                },
            ],
        };

        let mempool_snapshot = convert_to_mempool_snapshot(test_snapshot);

        assert_eq!(mempool_snapshot.block_height, 850000);
        assert!(!mempool_snapshot.bucketed_weights.is_empty());

        Ok(())
    }
}
