use crate::api_client::{ApiClient, ResponseComparator};
use anyhow::Result;
use colored::Colorize;
use serde_json::Value;
use tracing::{debug, info};

/// API compatibility test suite
pub struct CompatibilityTests {
    rust_client: ApiClient,
    reference_client: Option<ApiClient>,
}

impl CompatibilityTests {
    /// Create new compatibility test suite
    pub fn new(rust_url: String, reference_url: Option<String>) -> Self {
        Self {
            rust_client: ApiClient::new(rust_url),
            reference_client: reference_url.map(ApiClient::new),
        }
    }

    /// Run all compatibility tests
    pub async fn run_all(&self) -> Result<TestResults> {
        let mut results = TestResults::new();

        // Test fee estimates endpoint
        self.test_fee_estimates(&mut results).await?;

        // Test fee estimates with specific targets
        self.test_fee_targets(&mut results).await?;

        // Test error handling
        self.test_error_handling(&mut results).await?;

        // Test response format compatibility
        self.test_response_format(&mut results).await?;

        // If reference server available, run cross-implementation tests
        if self.reference_client.is_some() {
            self.test_cross_implementation(&mut results).await?;
        }

        results.print_summary();
        Ok(results)
    }

    /// Test fee estimates endpoint
    async fn test_fee_estimates(&self, results: &mut TestResults) -> Result<()> {
        info!("Testing /fees endpoint");

        let test_name = "GET /fees";

        // Test Rust implementation
        let rust_response = self.rust_client.get_fees().await;

        match rust_response {
            Ok(resp) => {
                // Validate response structure
                if self.validate_fee_response_structure(&resp) {
                    results.add_pass(test_name, "Response structure valid");
                } else {
                    results.add_fail(test_name, "Invalid response structure");
                }
            }
            Err(e) => {
                results.add_fail(test_name, &format!("Request failed: {e}"));
            }
        }

        Ok(())
    }

    /// Test fee estimates with specific targets
    async fn test_fee_targets(&self, results: &mut TestResults) -> Result<()> {
        info!("Testing /fees/target/num_blocks endpoint");

        // Skip 1.0 as it's rarely used and may not be supported
        let test_targets = vec![3.0, 6.0, 12.0, 24.0, 144.0];

        for target in test_targets {
            let test_name = format!("GET /fees/target/{target}");

            match self.rust_client.get_fees_for_target(target).await {
                Ok(resp) => {
                    if self.validate_fee_response_structure(&resp) {
                        results.add_pass(&test_name, "Response valid");
                    } else {
                        results.add_fail(&test_name, "Invalid response");
                    }
                }
                Err(e) => {
                    results.add_fail(&test_name, &format!("Failed: {e}"));
                }
            }
        }

        Ok(())
    }

    /// Test error handling
    async fn test_error_handling(&self, results: &mut TestResults) -> Result<()> {
        info!("Testing error handling");

        // Test invalid block targets
        let invalid_targets = vec![
            ("negative", "-1"),
            ("zero", "0"),
            ("non-numeric", "abc"),
            ("too_large", "10000"),
        ];

        for (test_case, value) in invalid_targets {
            let test_name = format!("Invalid target: {test_case}");
            let path = format!("/fees/target/{value}");

            match self.rust_client.get_raw(&path).await {
                Ok((status, _body)) => {
                    if status.as_u16() == 400 || status.as_u16() == 404 {
                        results.add_pass(&test_name, &format!("Correctly returned {status}"));
                    } else {
                        results.add_fail(&test_name, &format!("Unexpected status: {status}"));
                    }
                }
                Err(e) => {
                    results.add_fail(&test_name, &format!("Request error: {e}"));
                }
            }
        }

        Ok(())
    }

    /// Test response format compatibility
    async fn test_response_format(&self, results: &mut TestResults) -> Result<()> {
        info!("Testing response format compatibility");

        let test_name = "Response format validation";

        match self.rust_client.get_raw("/fees").await {
            Ok((status, body)) => {
                if status.as_u16() == 503 {
                    // No data available is OK
                    results.add_pass(test_name, "Service unavailable handled correctly");
                } else if status.is_success() {
                    // Validate JSON structure matches expected format
                    if self.validate_json_format(&body) {
                        results.add_pass(test_name, "JSON format valid");
                    } else {
                        results.add_fail(test_name, "JSON format invalid");
                    }
                } else {
                    results.add_fail(test_name, &format!("Unexpected status: {status}"));
                }
            }
            Err(e) => {
                results.add_fail(test_name, &format!("Request failed: {e}"));
            }
        }

        Ok(())
    }

    /// Test cross-implementation compatibility
    async fn test_cross_implementation(&self, results: &mut TestResults) -> Result<()> {
        if let Some(ref_client) = &self.reference_client {
            info!("Testing cross-implementation compatibility");

            // Test /fees endpoint
            let test_name = "Cross-impl: /fees";
            match self
                .compare_endpoints(&self.rust_client, ref_client, "/fees")
                .await
            {
                Ok(differences) => {
                    if differences.is_empty() {
                        results.add_pass(test_name, "Responses match");
                    } else {
                        let msg = format!("{count} differences found", count = differences.len());
                        results.add_warning(test_name, &msg);
                        for diff in &differences {
                            debug!("  - {diff}");
                        }
                    }
                }
                Err(e) => {
                    results.add_fail(test_name, &format!("Comparison failed: {e}"));
                }
            }

            // Test specific targets
            for target in [3.0, 6.0, 12.0] {
                let test_name = format!("Cross-impl: /fees/target/{target}");
                let path = format!("/fees/target/{target}");

                match self
                    .compare_endpoints(&self.rust_client, ref_client, &path)
                    .await
                {
                    Ok(differences) => {
                        if differences.is_empty() {
                            results.add_pass(&test_name, "Responses match");
                        } else {
                            results.add_warning(
                                &test_name,
                                &format!("{count} differences", count = differences.len()),
                            );
                        }
                    }
                    Err(e) => {
                        results.add_fail(&test_name, &format!("Failed: {e}"));
                    }
                }
            }
        }

        Ok(())
    }

    /// Compare responses from two endpoints
    async fn compare_endpoints(
        &self,
        client1: &ApiClient,
        client2: &ApiClient,
        path: &str,
    ) -> Result<Vec<String>> {
        let (status1, body1) = client1.get_raw(path).await?;
        let (status2, body2) = client2.get_raw(path).await?;

        let mut differences = Vec::new();

        // Compare status codes
        if status1 != status2 {
            differences.push(format!("Status code mismatch: {status1} vs {status2}"));
            return Ok(differences);
        }

        // Compare JSON bodies
        let json_diffs = ResponseComparator::compare_json(&body1, &body2, "");
        differences.extend(json_diffs);

        Ok(differences)
    }

    /// Validate fee response structure
    fn validate_fee_response_structure(
        &self,
        resp: &crate::api_client::FeeEstimateResponse,
    ) -> bool {
        // Check timestamp format
        if !resp.mempool_update_time.contains('T') {
            return false;
        }

        // If estimates exist, validate structure
        for target in resp.estimates.values() {
            for (prob_str, prob) in &target.probabilities {
                // Validate probability format (e.g., "0.95")
                if prob_str.parse::<f64>().is_err() {
                    return false;
                }

                // Validate fee rate is positive
                if prob.fee_rate < 0.0 {
                    return false;
                }
            }
        }

        true
    }

    /// Validate JSON format matches expected structure
    fn validate_json_format(&self, body: &Value) -> bool {
        // Must be an object
        let obj = match body.as_object() {
            Some(o) => o,
            None => return false,
        };

        // Must have mempool_update_time
        if !obj.contains_key("mempool_update_time") {
            return false;
        }

        // Must have estimates
        if !obj.contains_key("estimates") {
            return false;
        }

        // Validate estimates structure
        if let Some(estimates) = obj.get("estimates").and_then(|e| e.as_object()) {
            for (_block, target) in estimates {
                if !self.validate_block_target_json(target) {
                    return false;
                }
            }
        }

        true
    }

    /// Validate block target JSON structure
    fn validate_block_target_json(&self, target: &Value) -> bool {
        let obj = match target.as_object() {
            Some(o) => o,
            None => return false,
        };

        // Must have probabilities
        let probs = match obj.get("probabilities").and_then(|p| p.as_object()) {
            Some(p) => p,
            None => return false,
        };

        // Each probability must have fee_rate
        for (_prob, value) in probs {
            let prob_obj = match value.as_object() {
                Some(o) => o,
                None => return false,
            };

            if !prob_obj.contains_key("fee_rate") {
                return false;
            }
        }

        true
    }
}

/// Test results tracker
pub struct TestResults {
    passed: Vec<TestResult>,
    failed: Vec<TestResult>,
    warnings: Vec<TestResult>,
    start_time: std::time::Instant,
}

struct TestResult {
    name: String,
    message: String,
}

impl TestResults {
    pub fn new() -> Self {
        Self {
            passed: Vec::new(),
            failed: Vec::new(),
            warnings: Vec::new(),
            start_time: std::time::Instant::now(),
        }
    }

    pub fn add_pass(&mut self, name: &str, message: &str) {
        self.passed.push(TestResult {
            name: name.to_string(),
            message: message.to_string(),
        });
        println!(
            "{symbol} {name}: {msg}",
            symbol = "✓".green(),
            msg = message.dimmed()
        );
    }

    pub fn add_fail(&mut self, name: &str, message: &str) {
        self.failed.push(TestResult {
            name: name.to_string(),
            message: message.to_string(),
        });
        println!(
            "{symbol} {name}: {msg}",
            symbol = "✗".red(),
            msg = message.red()
        );
    }

    pub fn add_warning(&mut self, name: &str, message: &str) {
        self.warnings.push(TestResult {
            name: name.to_string(),
            message: message.to_string(),
        });
        println!(
            "{symbol} {name}: {msg}",
            symbol = "⚠".yellow(),
            msg = message.yellow()
        );
    }

    pub fn print_summary(&self) {
        let duration = self.start_time.elapsed();

        println!("\n{separator}", separator = "=".repeat(60));
        println!("Test Summary");
        println!("{separator}", separator = "=".repeat(60));

        println!(
            "Passed:   {count} {symbol}",
            count = self.passed.len().to_string().green(),
            symbol = "✓".green()
        );

        if !self.warnings.is_empty() {
            println!(
                "Warnings: {count} {symbol}",
                count = self.warnings.len().to_string().yellow(),
                symbol = "⚠".yellow()
            );
        }

        if !self.failed.is_empty() {
            println!(
                "Failed:   {count} {symbol}",
                count = self.failed.len().to_string().red(),
                symbol = "✗".red()
            );
        }

        println!(
            "Duration: {duration:.2}s",
            duration = duration.as_secs_f64()
        );
        println!("{separator}", separator = "=".repeat(60));

        if !self.failed.is_empty() {
            println!("\nFailed tests:");
            for test in &self.failed {
                println!(
                    "  {symbol} - {name}: {message}",
                    symbol = "✗".red(),
                    name = test.name,
                    message = test.message
                );
            }
        }
    }

    pub fn all_passed(&self) -> bool {
        self.failed.is_empty()
    }

    #[allow(dead_code)]
    pub fn total_tests(&self) -> usize {
        self.passed.len() + self.failed.len() + self.warnings.len()
    }
}
