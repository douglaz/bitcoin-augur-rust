use statrs::distribution::{DiscreteCDF, Poisson};

#[test]
fn test_poisson_calculation_logic() {
    println!("\n=== Testing Poisson Distribution Logic ===\n");

    // Test for target = 3 blocks
    let target = 3.0;
    let poisson = Poisson::new(target).unwrap();

    println!("Target: {} blocks", target);

    for &probability in &[0.05, 0.20, 0.50, 0.80, 0.95] {
        println!("\nProbability: {:.0}%", probability * 100.0);

        // Current algorithm logic (what's in the code now)
        let mut expected_blocks_current = 0.0;
        let max_search = (target * 4.0) as usize;

        for k in 0..max_search {
            let prob_at_least_k = 1.0 - poisson.cdf(k as u64);

            if prob_at_least_k < probability {
                if k > 0 {
                    expected_blocks_current = (k - 1) as f64;
                }
                println!(
                    "  Current algorithm: k={}, P(X >= {}) = {:.4}",
                    k, k, prob_at_least_k
                );
                println!("  Returns: {} expected blocks", expected_blocks_current);
                break;
            }
        }

        // The problem analysis:
        println!("  PROBLEM:");
        if probability == 0.95 && expected_blocks_current == 0.0 {
            println!("    For 95% confidence, algorithm returns 0 blocks!");
            println!("    This means we simulate 0 blocks being mined.");
            println!("    With 0 blocks mined, we get the minimum fee (1.00 sat/vB)");
        }

        // What the logic SHOULD be:
        // For 95% confidence, we want to find k such that:
        // P(X <= k) >= 0.95 (we're 95% confident that at most k blocks will be mined)
        // This is conservative - we prepare for more blocks being mined

        let mut corrected_blocks = 0.0;
        for k in 0..max_search {
            let prob_at_most_k = poisson.cdf(k as u64);
            if prob_at_most_k >= probability {
                corrected_blocks = k as f64;
                println!(
                    "  CORRECTED: k={}, P(X <= {}) = {:.4}",
                    k, k, prob_at_most_k
                );
                println!("  Should return: {} expected blocks", corrected_blocks);
                break;
            }
        }

        println!(
            "  Difference: {} blocks -> {} blocks",
            expected_blocks_current, corrected_blocks
        );
    }

    // The key insight:
    // - For LOW confidence (5%), we expect FEW blocks (optimistic, low fee is OK)
    // - For HIGH confidence (95%), we expect MANY blocks (conservative, need high fee)
    // The current code has this BACKWARDS!
}
