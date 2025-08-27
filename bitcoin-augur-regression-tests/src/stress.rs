//! Stress and concurrent testing module
//!
//! Tests server behavior under concurrent load and stress conditions

use anyhow::Result;
use colored::*;
use futures::future::join_all;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::info;

use crate::api_client::ApiClient;

/// Stress test configuration
#[derive(Debug, Clone)]
pub struct StressTestConfig {
    pub concurrent_requests: usize,
    pub iterations: usize,
    pub request_delay_ms: Option<u64>,
    pub endpoints: Vec<String>,
}

impl Default for StressTestConfig {
    fn default() -> Self {
        Self {
            concurrent_requests: 10,
            iterations: 5,
            request_delay_ms: None,
            endpoints: vec![
                "/fees".to_string(),
                "/fees/target/3".to_string(),
                "/fees/target/6".to_string(),
                "/fees/target/144".to_string(),
            ],
        }
    }
}

/// Stress test results
pub struct StressTestResults {
    pub total_requests: usize,
    pub successful_requests: usize,
    pub failed_requests: usize,
    pub average_response_time_ms: u128,
    pub min_response_time_ms: u128,
    pub max_response_time_ms: u128,
    pub requests_per_second: f64,
}

impl StressTestResults {
    pub fn print_summary(&self) {
        println!("\n{}", "Stress Test Results".bold());
        println!("{}", "===================".dimmed());

        let success_rate = (self.successful_requests as f64 / self.total_requests as f64) * 100.0;

        println!("Total requests:     {}", self.total_requests);
        println!(
            "Successful:         {} ({:.1}%)",
            self.successful_requests.to_string().green(),
            success_rate
        );

        if self.failed_requests > 0 {
            println!(
                "Failed:             {} ({:.1}%)",
                self.failed_requests.to_string().red(),
                100.0 - success_rate
            );
        }

        println!("\n{}", "Performance Metrics".bold());
        println!("{}", "-------------------".dimmed());
        println!("Average response:   {} ms", self.average_response_time_ms);
        println!("Min response:       {} ms", self.min_response_time_ms);
        println!("Max response:       {} ms", self.max_response_time_ms);
        println!("Requests/second:    {:.2}", self.requests_per_second);
    }
}

/// Run concurrent stress tests on a server
pub async fn run_stress_test(
    base_url: String,
    config: StressTestConfig,
) -> Result<StressTestResults> {
    info!(
        "Starting stress test: {} concurrent requests, {} iterations",
        config.concurrent_requests, config.iterations
    );

    let client = Arc::new(ApiClient::new(base_url));
    let mut all_response_times = Vec::new();
    let mut successful = 0;
    let mut failed = 0;

    let test_start = Instant::now();

    for iteration in 0..config.iterations {
        if iteration > 0 {
            if let Some(delay) = config.request_delay_ms {
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
        }

        // Create concurrent requests
        let mut futures = Vec::new();

        for _ in 0..config.concurrent_requests {
            let client = client.clone();
            let endpoint = config.endpoints[iteration % config.endpoints.len()].clone();

            futures.push(async move {
                let start = Instant::now();
                let result = make_request(&client, &endpoint).await;
                let duration = start.elapsed();
                (result, duration)
            });
        }

        // Execute all requests concurrently
        let results = join_all(futures).await;

        // Process results
        for (result, duration) in results {
            all_response_times.push(duration.as_millis());

            match result {
                Ok(_) => successful += 1,
                Err(_) => failed += 1,
            }
        }
    }

    let test_duration = test_start.elapsed();
    let total_requests = config.concurrent_requests * config.iterations;

    // Calculate statistics
    let average_response_time = if all_response_times.is_empty() {
        0
    } else {
        all_response_times.iter().sum::<u128>() / all_response_times.len() as u128
    };

    let min_response_time = all_response_times.iter().min().copied().unwrap_or(0);
    let max_response_time = all_response_times.iter().max().copied().unwrap_or(0);

    let requests_per_second = if test_duration.as_secs_f64() > 0.0 {
        total_requests as f64 / test_duration.as_secs_f64()
    } else {
        0.0
    };

    Ok(StressTestResults {
        total_requests,
        successful_requests: successful,
        failed_requests: failed,
        average_response_time_ms: average_response_time,
        min_response_time_ms: min_response_time,
        max_response_time_ms: max_response_time,
        requests_per_second,
    })
}

/// Make a request to an endpoint
async fn make_request(client: &ApiClient, endpoint: &str) -> Result<()> {
    match endpoint {
        "/fees" => {
            client.get_fees().await?;
        }
        path if path.starts_with("/fees/target/") => {
            let num_blocks = path
                .strip_prefix("/fees/target/")
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(6.0);
            client.get_fees_for_target(num_blocks).await?;
        }
        _ => {
            // Generic GET request
            client.get_raw(endpoint).await?;
        }
    }
    Ok(())
}

/// Performance comparison between two servers
pub async fn compare_performance(
    server1_url: String,
    server2_url: String,
    config: StressTestConfig,
) -> Result<()> {
    println!("\n{}", "Performance Comparison".bold().cyan());
    println!("{}", "======================".cyan());

    // Test first server
    println!("\n{}", "Testing Server 1...".yellow());
    let results1 = run_stress_test(server1_url, config.clone()).await?;

    // Test second server
    println!("\n{}", "Testing Server 2...".yellow());
    let results2 = run_stress_test(server2_url, config).await?;

    // Print comparison
    println!("\n{}", "Comparison Results".bold());
    println!("{}", "==================".dimmed());

    println!("\n{:<20} {:>15} {:>15}", "", "Server 1", "Server 2");
    println!("{:-<50}", "");

    println!(
        "{:<20} {:>15} {:>15}",
        "Success Rate",
        format!(
            "{:.1}%",
            results1.successful_requests as f64 / results1.total_requests as f64 * 100.0
        ),
        format!(
            "{:.1}%",
            results2.successful_requests as f64 / results2.total_requests as f64 * 100.0
        )
    );

    println!(
        "{:<20} {:>15} ms {:>15} ms",
        "Avg Response", results1.average_response_time_ms, results2.average_response_time_ms
    );

    println!(
        "{:<20} {:>15} ms {:>15} ms",
        "Min Response", results1.min_response_time_ms, results2.min_response_time_ms
    );

    println!(
        "{:<20} {:>15} ms {:>15} ms",
        "Max Response", results1.max_response_time_ms, results2.max_response_time_ms
    );

    println!(
        "{:<20} {:>15.2} {:>15.2}",
        "Requests/sec", results1.requests_per_second, results2.requests_per_second
    );

    // Determine winner
    let server1_faster = results1.average_response_time_ms < results2.average_response_time_ms;
    let speed_ratio = if server1_faster {
        results2.average_response_time_ms as f64 / results1.average_response_time_ms as f64
    } else {
        results1.average_response_time_ms as f64 / results2.average_response_time_ms as f64
    };

    println!("\n{}", "Summary".bold());
    println!("{}", "-------".dimmed());

    if server1_faster {
        println!(
            "Server 1 is {:.1}x faster than Server 2",
            speed_ratio.to_string().green()
        );
    } else {
        println!(
            "Server 2 is {:.1}x faster than Server 1",
            speed_ratio.to_string().green()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stress_config_default() {
        let config = StressTestConfig::default();
        assert_eq!(config.concurrent_requests, 10);
        assert_eq!(config.iterations, 5);
        assert_eq!(config.endpoints.len(), 4);
    }
}
