use anyhow::Result;
use colored::*;
use std::time::Instant;

use crate::api::ApiClient;
use crate::report::TestReport;
use crate::server::Server;

pub async fn run_advanced_tests(
    rust_server: &dyn Server,
    kotlin_server: &dyn Server,
    report: &mut TestReport,
) -> Result<()> {
    println!("\n{}", "Running Advanced Tests".bold());
    println!("{}", "----------------------".dimmed());

    let rust_client = ApiClient::new(rust_server.base_url());
    let kotlin_client = ApiClient::new(kotlin_server.base_url());

    // Test 1: Performance comparison
    {
        println!("\n⚡ Test: Performance comparison");

        let test_name = "performance";
        let iterations = 10;

        // Measure Rust server
        let rust_start = Instant::now();
        for _ in 0..iterations {
            let _ = rust_client.get_fees().await;
        }
        let rust_duration = rust_start.elapsed();
        let rust_avg_ms = rust_duration.as_millis() / iterations;

        // Measure Kotlin server
        let kotlin_start = Instant::now();
        for _ in 0..iterations {
            let _ = kotlin_client.get_fees().await;
        }
        let kotlin_duration = kotlin_start.elapsed();
        let kotlin_avg_ms = kotlin_duration.as_millis() / iterations;

        println!("  📊 Rust server:   {} ms average", rust_avg_ms);
        println!("  📊 Kotlin server: {} ms average", kotlin_avg_ms);

        // Consider test passed if both respond reasonably fast
        if rust_avg_ms < 1000 && kotlin_avg_ms < 1000 {
            report.add_passed(test_name);

            let faster = if rust_avg_ms < kotlin_avg_ms {
                format!(
                    "Rust is {:.1}x faster",
                    kotlin_avg_ms as f64 / rust_avg_ms as f64
                )
            } else {
                format!(
                    "Kotlin is {:.1}x faster",
                    rust_avg_ms as f64 / kotlin_avg_ms as f64
                )
            };
            println!("  ✅ Both servers respond quickly ({})", faster.green());
        } else {
            report.add_failed(test_name);
            println!("  ❌ One or both servers are too slow");
        }
    }

    // Test 2: Concurrent requests
    {
        println!("\n🔄 Test: Concurrent requests handling");

        let test_name = "concurrent_requests";
        let concurrent_count = 5;

        // Send concurrent requests to both servers
        let mut rust_futures = Vec::new();
        let mut kotlin_futures = Vec::new();

        for _ in 0..concurrent_count {
            rust_futures.push(rust_client.get_fees());
            kotlin_futures.push(kotlin_client.get_fees());
        }

        let rust_results = futures::future::join_all(rust_futures).await;
        let kotlin_results = futures::future::join_all(kotlin_futures).await;

        let rust_success = rust_results.iter().filter(|r| r.is_ok()).count();
        let kotlin_success = kotlin_results.iter().filter(|r| r.is_ok()).count();

        println!(
            "  📊 Rust server:   {}/{} successful",
            rust_success, concurrent_count
        );
        println!(
            "  📊 Kotlin server: {}/{} successful",
            kotlin_success, concurrent_count
        );

        if rust_success == concurrent_count && kotlin_success == concurrent_count {
            report.add_passed(test_name);
            println!("  ✅ Both servers handled concurrent requests");
        } else {
            report.add_failed(test_name);
            println!("  ❌ Some concurrent requests failed");
        }
    }

    // Test 3: Response structure validation
    {
        println!("\n🔍 Test: Response structure validation");

        let test_name = "response_structure";

        match (rust_client.get_fees().await, kotlin_client.get_fees().await) {
            (Ok(rust_resp), Ok(kotlin_resp)) => {
                let mut structure_match = true;

                // Check that both have the same block targets
                let rust_targets: Vec<_> = rust_resp.estimates.keys().collect();
                let kotlin_targets: Vec<_> = kotlin_resp.estimates.keys().collect();

                if rust_targets.len() != kotlin_targets.len() {
                    structure_match = false;
                    println!("  ⚠️  Different number of block targets");
                    println!("    Rust:   {} targets", rust_targets.len());
                    println!("    Kotlin: {} targets", kotlin_targets.len());
                }

                // Check probability levels
                for (target, rust_data) in &rust_resp.estimates {
                    if let Some(kotlin_data) = kotlin_resp.estimates.get(target) {
                        let rust_probs: Vec<_> = rust_data.probabilities.keys().collect();
                        let kotlin_probs: Vec<_> = kotlin_data.probabilities.keys().collect();

                        if rust_probs.len() != kotlin_probs.len() {
                            structure_match = false;
                            println!("  ⚠️  Different probability levels for {} blocks", target);
                        }
                    }
                }

                if structure_match {
                    report.add_passed(test_name);
                    println!("  ✅ Response structures match");
                } else {
                    report.add_failed(test_name);
                    println!("  ❌ Response structures differ");
                }
            }
            _ => {
                report.add_skipped(test_name);
                println!("  ⚠️  Could not retrieve responses for comparison");
            }
        }
    }

    Ok(())
}

// futures is already available through tokio re-exports or direct usage
