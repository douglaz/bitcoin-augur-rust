use anyhow::Result;
use colored::*;

use crate::api::ApiClient;
use crate::comparison::compare_fee_responses;
use crate::report::TestReport;
use crate::server::Server;

pub async fn run_basic_comparison_tests(
    rust_server: &dyn Server,
    kotlin_server: &dyn Server,
    report: &mut TestReport,
) -> Result<()> {
    println!("\n{}", "Running Basic Comparison Tests".bold());
    println!("{}", "-------------------------------".dimmed());

    let rust_client = ApiClient::new(rust_server.base_url());
    let kotlin_client = ApiClient::new(kotlin_server.base_url());

    // Test 1: Compare /fees endpoint
    {
        println!("\n📊 Test: Compare /fees endpoint");

        let test_name = "fees_endpoint";

        match (rust_client.get_fees().await, kotlin_client.get_fees().await) {
            (Ok(rust_resp), Ok(kotlin_resp)) => {
                let diff_result = compare_fee_responses(&rust_resp, &kotlin_resp);

                if diff_result.passed {
                    report.add_passed(test_name);
                    println!("  ✅ {}", "Responses match".green());
                } else {
                    report.add_failed(test_name);
                    diff_result.print_summary("  Fee estimates comparison");
                }
            }
            (Err(e), _) => {
                report.add_failed(test_name);
                println!("  ❌ Rust server error: {}", e.to_string().red());
            }
            (_, Err(e)) => {
                report.add_failed(test_name);
                println!("  ❌ Kotlin server error: {}", e.to_string().red());
            }
        }
    }

    // Test 2: Compare specific block targets
    let block_targets = vec![3, 6, 12, 24, 144];

    for blocks in block_targets {
        println!("\n📊 Test: Compare /fees/target/{} endpoint", blocks);

        let test_name = format!("fees_target_{}", blocks);

        match (
            rust_client.get_fee_for_target(blocks).await,
            kotlin_client.get_fee_for_target(blocks).await,
        ) {
            (Ok(rust_resp), Ok(kotlin_resp)) => {
                let diff_result = compare_fee_responses(&rust_resp, &kotlin_resp);

                if diff_result.passed {
                    report.add_passed(&test_name);
                    println!("  ✅ {} blocks: {}", blocks, "Responses match".green());
                } else {
                    report.add_failed(&test_name);
                    diff_result.print_summary(&format!("  {} blocks comparison", blocks));
                }
            }
            (Err(e), _) => {
                // Check if it's a 404 (endpoint might not exist)
                if e.to_string().contains("404") {
                    report.add_skipped(&test_name);
                    println!(
                        "  ⚠️  {} blocks: Endpoint not implemented in Rust server",
                        blocks
                    );
                } else {
                    report.add_failed(&test_name);
                    println!(
                        "  ❌ {} blocks: Rust server error: {}",
                        blocks,
                        e.to_string().red()
                    );
                }
            }
            (_, Err(e)) => {
                if e.to_string().contains("404") {
                    report.add_skipped(&test_name);
                    println!(
                        "  ⚠️  {} blocks: Endpoint not implemented in Kotlin server",
                        blocks
                    );
                } else {
                    report.add_failed(&test_name);
                    println!(
                        "  ❌ {} blocks: Kotlin server error: {}",
                        blocks,
                        e.to_string().red()
                    );
                }
            }
        }
    }

    Ok(())
}

pub async fn run_single_server_tests(server: &dyn Server, report: &mut TestReport) -> Result<()> {
    println!("\n{}", "Running Single Server Tests".bold());
    println!("{}", "----------------------------".dimmed());

    let client = ApiClient::new(server.base_url());

    // Test 1: Basic /fees endpoint
    {
        println!("\n📊 Test: {} /fees endpoint", server.name());

        let test_name = "single_fees_endpoint";

        match client.get_fees().await {
            Ok(resp) => {
                report.add_passed(test_name);
                println!("  ✅ Successfully retrieved fee estimates");

                // Print some basic info
                println!("  📈 Block targets found: {}", resp.estimates.len());
                for (target, data) in resp.estimates.iter().take(3) {
                    println!(
                        "    • {} blocks: {} probabilities",
                        target,
                        data.probabilities.len()
                    );
                }
            }
            Err(e) => {
                // Check if it's because no data yet
                if e.to_string().contains("503") {
                    report.add_skipped(test_name);
                    println!("  ⚠️  No mempool data collected yet");
                } else {
                    report.add_failed(test_name);
                    println!("  ❌ Error: {}", e.to_string().red());
                }
            }
        }
    }

    // Test 2: Health check
    {
        println!("\n📊 Test: {} health check", server.name());

        let test_name = "single_health_check";

        match client.health_check().await {
            Ok(true) => {
                report.add_passed(test_name);
                println!("  ✅ Server is healthy");
            }
            Ok(false) => {
                report.add_failed(test_name);
                println!("  ❌ Server returned unhealthy status");
            }
            Err(e) => {
                report.add_failed(test_name);
                println!("  ❌ Health check failed: {}", e.to_string().red());
            }
        }
    }

    Ok(())
}
