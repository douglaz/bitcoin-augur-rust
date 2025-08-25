use colored::*;
use std::collections::HashMap;

#[derive(Debug)]
pub struct TestReport {
    pub rust_server_started: bool,
    pub kotlin_server_started: bool,
    pub tests: HashMap<String, TestStatus>,
}

#[derive(Debug, Clone, Copy)]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
}

impl TestReport {
    pub fn new() -> Self {
        Self {
            rust_server_started: false,
            kotlin_server_started: false,
            tests: HashMap::new(),
        }
    }

    pub fn add_passed(&mut self, test_name: &str) {
        self.tests.insert(test_name.to_string(), TestStatus::Passed);
    }

    pub fn add_failed(&mut self, test_name: &str) {
        self.tests.insert(test_name.to_string(), TestStatus::Failed);
    }

    pub fn add_skipped(&mut self, test_name: &str) {
        self.tests
            .insert(test_name.to_string(), TestStatus::Skipped);
    }

    pub fn all_passed(&self) -> bool {
        self.tests
            .values()
            .all(|status| matches!(status, TestStatus::Passed | TestStatus::Skipped))
    }

    pub fn print_summary(&self) {
        println!("\n{}", "═".repeat(60).cyan());
        println!("{}", "Test Summary".bold().cyan());
        println!("{}", "═".repeat(60).cyan());

        // Server status
        println!("\n{}", "Server Status:".bold());
        if self.rust_server_started {
            println!("  ✅ Rust server started successfully");
        } else {
            println!("  ⚠️  Rust server not started");
        }

        if self.kotlin_server_started {
            println!("  ✅ Kotlin server started successfully");
        } else {
            println!("  ⚠️  Kotlin server not started");
        }

        // Test results
        let passed = self
            .tests
            .values()
            .filter(|s| matches!(s, TestStatus::Passed))
            .count();
        let failed = self
            .tests
            .values()
            .filter(|s| matches!(s, TestStatus::Failed))
            .count();
        let skipped = self
            .tests
            .values()
            .filter(|s| matches!(s, TestStatus::Skipped))
            .count();

        println!("\n{}", "Test Results:".bold());
        println!("  Total:   {}", self.tests.len());
        println!("  Passed:  {} {}", passed, "✅".green());
        println!("  Failed:  {} {}", failed, "❌".red());
        println!("  Skipped: {} {}", skipped, "⚠️".yellow());

        // Individual test results
        if !self.tests.is_empty() {
            println!("\n{}", "Individual Tests:".bold());

            let mut test_names: Vec<_> = self.tests.keys().collect();
            test_names.sort();

            for test_name in test_names {
                let status = self.tests[test_name];
                let (symbol, color_fn): (&str, fn(&str) -> ColoredString) = match status {
                    TestStatus::Passed => ("✅", |s| s.green()),
                    TestStatus::Failed => ("❌", |s| s.red()),
                    TestStatus::Skipped => ("⚠️ ", |s| s.yellow()),
                };

                println!("  {} {}", symbol, color_fn(test_name));
            }
        }

        // Final verdict
        println!("\n{}", "═".repeat(60).cyan());
        if self.all_passed() {
            println!("{}", "✅ All tests passed!".bold().green());
        } else {
            println!("{}", "❌ Some tests failed!".bold().red());
        }
        println!("{}", "═".repeat(60).cyan());
    }
}
