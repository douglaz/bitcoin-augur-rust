use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use futures::future::join_all;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{error, info, warn};

use crate::{
    api_client::ApiClient,
    compatibility::{CompatibilityTests, TestResults as CompatTestResults},
    server::{ReferenceServerManager, ServerManager},
    snapshots::SnapshotTester,
    test_cases::{TestCase, TestCaseGenerator},
    test_vectors::{TestVector, TestVectorRunner},
};

/// Main test runner
pub struct TestRunner {
    server_path: Option<PathBuf>,
    reference_jar: Option<PathBuf>,
    port: Option<u16>,
    data_dir: PathBuf,
    update_snapshots: bool,
    filter: Option<String>,
    jobs: usize,
    server_manager: Option<ServerManager>,
    reference_manager: Option<ReferenceServerManager>,
}

impl TestRunner {
    /// Create new test runner
    pub fn new(
        server_path: Option<PathBuf>,
        reference_jar: Option<PathBuf>,
        port: Option<u16>,
        data_dir: PathBuf,
        update_snapshots: bool,
        filter: Option<String>,
        jobs: usize,
    ) -> Result<Self> {
        // Auto-detect server binary if not provided
        let server_path = match server_path {
            Some(p) => Some(p),
            None => Self::find_server_binary()?,
        };

        Ok(Self {
            server_path,
            reference_jar,
            port,
            data_dir,
            update_snapshots,
            filter,
            jobs,
            server_manager: None,
            reference_manager: None,
        })
    }

    /// Find server binary in workspace
    fn find_server_binary() -> Result<Option<PathBuf>> {
        let candidates = vec![
            "target/release/bitcoin-augur-server",
            "target/debug/bitcoin-augur-server",
            "../target/release/bitcoin-augur-server",
            "../target/debug/bitcoin-augur-server",
        ];

        for path_str in candidates {
            let path = PathBuf::from(path_str);
            if path.exists() && path.is_file() {
                info!("Found server binary at {:?}", path);
                return Ok(Some(path.canonicalize()?));
            }
        }

        warn!("No server binary found, will skip server-dependent tests");
        Ok(None)
    }

    /// Get an available port
    async fn get_available_port(&self) -> Result<u16> {
        if let Some(port) = self.port {
            return Ok(port);
        }

        // Find random available port
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        drop(listener);
        Ok(port)
    }

    /// Run all tests
    pub async fn run_all(
        &mut self,
        run_snapshots: bool,
        run_compatibility: bool,
        run_vectors: bool,
    ) -> Result<()> {
        info!("Starting regression test suite");
        
        let mut all_passed = true;

        // Start server if available
        if let Some(ref server_path) = self.server_path {
            let port = self.get_available_port().await?;
            self.start_server(server_path.clone(), port).await?;

            if run_snapshots {
                info!("Running snapshot tests");
                match self.run_snapshot_tests(false).await {
                    Ok(()) => info!("Snapshot tests completed"),
                    Err(e) => {
                        error!("Snapshot tests failed: {}", e);
                        all_passed = false;
                    }
                }
            }

            if run_compatibility {
                info!("Running compatibility tests");
                match self.run_compatibility_tests(false).await {
                    Ok(()) => info!("Compatibility tests completed"),
                    Err(e) => {
                        error!("Compatibility tests failed: {}", e);
                        all_passed = false;
                    }
                }
            }
        } else {
            warn!("Server binary not available, skipping server-dependent tests");
        }

        if run_vectors {
            info!("Running test vector validation");
            match self.run_vector_tests(None).await {
                Ok(()) => info!("Test vector validation completed"),
                Err(e) => {
                    error!("Test vector validation failed: {}", e);
                    all_passed = false;
                }
            }
        }

        // Stop servers
        self.stop_servers().await?;

        if all_passed {
            println!("\n{}", "All tests passed!".green().bold());
            Ok(())
        } else {
            Err(anyhow!("Some tests failed"))
        }
    }

    /// Run compatibility tests
    pub async fn run_compatibility_tests(&mut self, with_reference: bool) -> Result<()> {
        // Ensure server is running
        if self.server_manager.is_none() {
            if let Some(ref server_path) = self.server_path {
                let port = self.get_available_port().await?;
                self.start_server(server_path.clone(), port).await?;
            } else {
                return Err(anyhow!("No server binary available"));
            }
        }

        let rust_url = self
            .server_manager
            .as_ref()
            .map(|m| m.url())
            .ok_or_else(|| anyhow!("Rust server not running"))?;

        let reference_url = if with_reference {
            if let Some(ref jar_path) = self.reference_jar {
                let port = self.get_available_port().await?;
                self.start_reference_server(jar_path.clone(), port).await?;
                Some(
                    self.reference_manager
                        .as_ref()
                        .map(|m| m.url())
                        .ok_or_else(|| anyhow!("Reference server not running"))?,
                )
            } else {
                warn!("Reference JAR not provided, skipping cross-implementation tests");
                None
            }
        } else {
            None
        };

        let compat_tests = CompatibilityTests::new(rust_url, reference_url);
        let results = compat_tests.run_all().await?;

        if !results.all_passed() {
            return Err(anyhow!("Compatibility tests failed"));
        }

        Ok(())
    }

    /// Run snapshot tests
    pub async fn run_snapshot_tests(&mut self, force_update: bool) -> Result<()> {
        // Ensure server is running
        if self.server_manager.is_none() {
            if let Some(ref server_path) = self.server_path {
                let port = self.get_available_port().await?;
                self.start_server(server_path.clone(), port).await?;
            } else {
                return Err(anyhow!("No server binary available"));
            }
        }

        let server_url = self
            .server_manager
            .as_ref()
            .map(|m| m.url())
            .ok_or_else(|| anyhow!("Server not running"))?;

        let tester = SnapshotTester::new(self.update_snapshots || force_update);
        let results = tester.run_tests(&server_url).await?;

        if !results.all_passed() {
            return Err(anyhow!("Snapshot tests failed"));
        }

        Ok(())
    }

    /// Run test vector validation
    pub async fn run_vector_tests(&mut self, vectors_file: Option<PathBuf>) -> Result<()> {
        let vectors = if let Some(path) = vectors_file {
            info!("Loading test vectors from {:?}", path);
            TestVectorRunner::load_vectors(&path).await?
        } else {
            info!("Using default test vectors");
            TestVectorRunner::generate_default_vectors()
        };

        info!("Running {} test vectors", vectors.len());

        let semaphore = Arc::new(Semaphore::new(self.jobs));
        let mut tasks = Vec::new();

        for vector in vectors {
            let sem = semaphore.clone();
            let filter = self.filter.clone();

            // Apply filter if provided
            if let Some(ref f) = filter {
                if !vector.name.contains(f) {
                    continue;
                }
            }

            tasks.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                TestVectorRunner::run_vector(&vector)
            }));
        }

        let results = join_all(tasks).await;
        
        let mut all_passed = true;
        for result in results {
            match result {
                Ok(Ok(test_result)) => {
                    test_result.print_summary();
                    if !test_result.passed {
                        all_passed = false;
                    }
                }
                Ok(Err(e)) => {
                    error!("Test vector failed: {}", e);
                    all_passed = false;
                }
                Err(e) => {
                    error!("Task failed: {}", e);
                    all_passed = false;
                }
            }
        }

        if !all_passed {
            return Err(anyhow!("Some test vectors failed"));
        }

        Ok(())
    }

    /// Generate test data
    pub async fn generate_test_data(&mut self, output: PathBuf, count: usize) -> Result<()> {
        info!("Generating {} test cases", count);

        // Generate test cases
        let test_cases = TestCaseGenerator::generate(count);

        // Save test cases
        let test_cases_path = output.join("test_cases.json");
        let json = serde_json::to_string_pretty(&test_cases)?;
        tokio::fs::create_dir_all(&output).await?;
        tokio::fs::write(&test_cases_path, json).await?;
        info!("Saved test cases to {:?}", test_cases_path);

        // Generate test vectors
        let vectors = TestVectorRunner::generate_default_vectors();
        let vectors_path = output.join("test_vectors.json");
        TestVectorRunner::save_vectors(&vectors, &vectors_path).await?;
        info!("Saved test vectors to {:?}", vectors_path);

        println!(
            "\n{}",
            format!("Generated {} test cases and {} test vectors", count, vectors.len())
                .green()
                .bold()
        );

        Ok(())
    }

    /// Compare two endpoints
    pub async fn compare_endpoints(
        &mut self,
        endpoint1: &str,
        endpoint2: &str,
        path: &str,
    ) -> Result<()> {
        info!("Comparing endpoints: {} vs {}", endpoint1, endpoint2);

        let client1 = ApiClient::new(endpoint1.to_string());
        let client2 = ApiClient::new(endpoint2.to_string());

        let (status1, body1) = client1.get_raw(path).await?;
        let (status2, body2) = client2.get_raw(path).await?;

        if status1 != status2 {
            println!(
                "{} Status codes differ: {} vs {}",
                "✗".red(),
                status1,
                status2
            );
            return Err(anyhow!("Status codes differ"));
        }

        let differences = crate::api_client::ResponseComparator::compare_json(&body1, &body2, "");

        if differences.is_empty() {
            println!("{} Responses are identical", "✓".green());
        } else {
            println!("{} Found {} differences:", "⚠".yellow(), differences.len());
            for diff in &differences {
                println!("  - {}", diff);
            }
        }

        Ok(())
    }

    /// Start the server
    async fn start_server(&mut self, binary_path: PathBuf, port: u16) -> Result<()> {
        let data_dir = self.data_dir.join("rust-server");
        tokio::fs::create_dir_all(&data_dir).await?;

        let mut manager = ServerManager::new(binary_path, port, data_dir);
        manager.start().await?;
        self.server_manager = Some(manager);
        Ok(())
    }

    /// Start the reference server
    async fn start_reference_server(&mut self, jar_path: PathBuf, port: u16) -> Result<()> {
        let data_dir = self.data_dir.join("reference-server");
        tokio::fs::create_dir_all(&data_dir).await?;

        let mut manager = ReferenceServerManager::new(jar_path, port, data_dir);
        manager.start().await?;
        self.reference_manager = Some(manager);
        Ok(())
    }

    /// Stop all servers
    async fn stop_servers(&mut self) -> Result<()> {
        if let Some(mut manager) = self.server_manager.take() {
            manager.stop().await?;
        }
        if let Some(mut manager) = self.reference_manager.take() {
            manager.stop().await?;
        }
        Ok(())
    }
}

impl Drop for TestRunner {
    fn drop(&mut self) {
        // Ensure servers are stopped
        if let Some(mut manager) = self.server_manager.take() {
            let _ = tokio::runtime::Runtime::new()
                .and_then(|rt| Ok(rt.block_on(manager.stop())));
        }
        if let Some(mut manager) = self.reference_manager.take() {
            let _ = tokio::runtime::Runtime::new()
                .and_then(|rt| Ok(rt.block_on(manager.stop())));
        }
    }
}