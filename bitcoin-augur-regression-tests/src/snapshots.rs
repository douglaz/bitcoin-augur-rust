#![allow(dead_code)]

use anyhow::{Context, Result};
use insta::{assert_json_snapshot, Settings};
use serde_json::Value;
use std::path::Path;
use tracing::{debug, info};

/// Snapshot testing for regression detection
pub struct SnapshotTester {
    update_snapshots: bool,
}

impl SnapshotTester {
    /// Create new snapshot tester
    pub fn new(update_snapshots: bool) -> Self {
        Self { update_snapshots }
    }

    /// Run snapshot tests
    pub async fn run_tests(&self, api_url: &str) -> Result<SnapshotTestResults> {
        let mut results = SnapshotTestResults::new();

        // Configure insta settings
        let mut settings = Settings::clone_current();
        settings.set_snapshot_path("snapshots");

        if self.update_snapshots {
            // Note: insta doesn't have set_update_snapshots method in this version
            // Will rely on INSTA_UPDATE environment variable instead
        }

        // Run the async tests directly without creating a new runtime
        let client = crate::api_client::ApiClient::new(api_url.to_string());

        // Test fee estimates snapshot
        info!("Testing fee estimates snapshot");

        let test_name = "fee_estimates";
        match client.get_fees().await {
            Ok(response) => {
                // Redact timestamp for consistent snapshots
                let mut value = serde_json::to_value(&response)?;
                Self::redact_timestamps(&mut value);

                // We need to use settings.bind for insta snapshots
                settings.bind(|| {
                    assert_json_snapshot!(test_name, value, {
                        ".mempool_update_time" => "[timestamp]"
                    });
                });

                results.add_pass(test_name);
            }
            Err(e) => {
                results.add_fail(test_name, &format!("Failed to get response: {e}"));
            }
        }

        // Test specific block targets (skip 1.0 as it's rarely used)
        for target in [3.0, 6.0, 12.0, 24.0, 144.0] {
            let test_name = format!("fee_estimates_target_{target}");

            match client.get_fees_for_target(target).await {
                Ok(response) => {
                    let mut value = serde_json::to_value(&response)?;
                    Self::redact_timestamps(&mut value);

                    settings.bind(|| {
                        assert_json_snapshot!(test_name.as_str(), value, {
                            ".mempool_update_time" => "[timestamp]"
                        });
                    });

                    results.add_pass(&test_name);
                }
                Err(e) => {
                    results.add_fail(&test_name, &format!("Failed: {e}"));
                }
            }
        }

        results.print_summary();
        Ok(results)
    }

    /// Redact timestamps for consistent snapshots
    fn redact_timestamps(value: &mut Value) {
        match value {
            Value::Object(map) => {
                for (key, val) in map.iter_mut() {
                    if key.contains("time") || key.contains("timestamp") {
                        *val = Value::String("[timestamp]".to_string());
                    } else {
                        Self::redact_timestamps(val);
                    }
                }
            }
            Value::Array(arr) => {
                for item in arr {
                    Self::redact_timestamps(item);
                }
            }
            _ => {}
        }
    }

    /// Compare snapshots between two runs
    pub fn compare_snapshots(
        snapshot_dir1: &Path,
        snapshot_dir2: &Path,
    ) -> Result<Vec<SnapshotDifference>> {
        let mut differences = Vec::new();

        // Read all snapshots from both directories
        let snapshots1 = Self::read_snapshot_dir(snapshot_dir1)?;
        let snapshots2 = Self::read_snapshot_dir(snapshot_dir2)?;

        // Compare each snapshot
        for (name, content1) in &snapshots1 {
            if let Some(content2) = snapshots2.get(name) {
                if content1 != content2 {
                    differences.push(SnapshotDifference {
                        name: name.clone(),
                        kind: DifferenceKind::Modified,
                        details: Self::get_json_diff(content1, content2),
                    });
                }
            } else {
                differences.push(SnapshotDifference {
                    name: name.clone(),
                    kind: DifferenceKind::Removed,
                    details: "Snapshot exists in first but not in second".to_string(),
                });
            }
        }

        // Check for new snapshots in second
        for name in snapshots2.keys() {
            if !snapshots1.contains_key(name) {
                differences.push(SnapshotDifference {
                    name: name.clone(),
                    kind: DifferenceKind::Added,
                    details: "Snapshot exists in second but not in first".to_string(),
                });
            }
        }

        Ok(differences)
    }

    /// Read all snapshots from directory
    fn read_snapshot_dir(dir: &Path) -> Result<std::collections::HashMap<String, Value>> {
        use std::collections::HashMap;

        let mut snapshots = HashMap::new();

        if !dir.exists() {
            return Ok(snapshots);
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("snap") {
                let content = std::fs::read_to_string(&path)?;
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                // Parse the snapshot content (insta format includes metadata)
                if let Ok(value) = Self::parse_snapshot_content(&content) {
                    snapshots.insert(name, value);
                }
            }
        }

        Ok(snapshots)
    }

    /// Parse snapshot content from insta format
    fn parse_snapshot_content(content: &str) -> Result<Value> {
        // Insta snapshots have a specific format with metadata
        // We need to extract the actual JSON content

        // Find the start of the JSON content (after the metadata)
        if let Some(json_start) = content.find("---\n") {
            let json_part = &content[json_start + 4..];
            serde_json::from_str(json_part).context("Failed to parse snapshot JSON")
        } else {
            // Try parsing the whole content as JSON
            serde_json::from_str(content).context("Failed to parse snapshot content")
        }
    }

    /// Get difference between two JSON values
    fn get_json_diff(val1: &Value, val2: &Value) -> String {
        use assert_json_diff::assert_json_matches_no_panic;

        match assert_json_matches_no_panic(
            val1,
            val2,
            assert_json_diff::Config::new(assert_json_diff::CompareMode::Strict),
        ) {
            Ok(_) => "Values are identical".to_string(),
            Err(e) => e.to_string(),
        }
    }
}

/// Snapshot test results
pub struct SnapshotTestResults {
    passed: Vec<String>,
    failed: Vec<(String, String)>,
}

impl SnapshotTestResults {
    pub fn new() -> Self {
        Self {
            passed: Vec::new(),
            failed: Vec::new(),
        }
    }

    pub fn add_pass(&mut self, name: &str) {
        self.passed.push(name.to_string());
        debug!("Snapshot test passed: {name}");
    }

    pub fn add_fail(&mut self, name: &str, reason: &str) {
        self.failed.push((name.to_string(), reason.to_string()));
        debug!("Snapshot test failed: {} - {}", name, reason);
    }

    pub fn print_summary(&self) {
        use colored::Colorize;

        println!("\n{}", "Snapshot Test Results".bold());
        println!("{}", "=".repeat(50));

        println!(
            "Passed: {} {}",
            self.passed.len().to_string().green(),
            "✓".green()
        );

        if !self.failed.is_empty() {
            println!(
                "Failed: {} {}",
                self.failed.len().to_string().red(),
                "✗".red()
            );

            println!("\nFailed tests:");
            for (name, reason) in &self.failed {
                println!("  {} {}: {}", "✗".red(), name, reason.dimmed());
            }
        }
    }

    pub fn all_passed(&self) -> bool {
        self.failed.is_empty()
    }
}

/// Snapshot difference
#[derive(Debug)]
pub struct SnapshotDifference {
    pub name: String,
    pub kind: DifferenceKind,
    pub details: String,
}

#[derive(Debug)]
pub enum DifferenceKind {
    Added,
    Removed,
    Modified,
}
