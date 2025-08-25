use crate::api::FeeEstimateResponse;
use float_cmp::approx_eq;

/// Extract fee rate from response for a specific target and probability
pub fn get_fee_rate(response: &FeeEstimateResponse, target: u32, probability: f64) -> Option<f64> {
    response
        .estimates
        .get(&target.to_string())
        .and_then(|t| t.probabilities.get(&format!("{:.2}", probability)))
        .map(|p| p.fee_rate)
}

/// Check if two floats are approximately equal within tolerance
pub fn fees_match(a: f64, b: f64, tolerance: f64) -> bool {
    approx_eq!(f64, a, b, epsilon = tolerance * a.max(b))
}

/// Default block targets used by Bitcoin Augur
pub const DEFAULT_BLOCK_TARGETS: &[u32] = &[3, 6, 12, 24, 144];

/// Default probability levels used by Bitcoin Augur
pub const DEFAULT_PROBABILITIES: &[f64] = &[0.05, 0.20, 0.50, 0.80, 0.95];

/// Compare two fee estimate responses and return detailed comparison
pub fn compare_responses(
    rust_resp: &FeeEstimateResponse,
    kotlin_resp: &FeeEstimateResponse,
    tolerance: f64,
) -> ComparisonResult {
    let mut result = ComparisonResult::default();

    // Check that both have same block targets
    let rust_targets: Vec<_> = rust_resp.estimates.keys().map(|k| k.as_str()).collect();
    let kotlin_targets: Vec<_> = kotlin_resp.estimates.keys().map(|k| k.as_str()).collect();

    if rust_targets.len() != kotlin_targets.len() {
        let rust_count = rust_targets.len();
        let kotlin_count = kotlin_targets.len();
        result.add_error(format!(
            "Different number of block targets: Rust={rust_count}, Kotlin={kotlin_count}"
        ));
    }

    // Compare fee rates for each target and probability
    for target in DEFAULT_BLOCK_TARGETS {
        for prob in DEFAULT_PROBABILITIES {
            let rust_fee = get_fee_rate(rust_resp, *target, *prob);
            let kotlin_fee = get_fee_rate(kotlin_resp, *target, *prob);

            match (rust_fee, kotlin_fee) {
                (Some(r), Some(k)) => {
                    if !fees_match(r, k, tolerance) {
                        let diff_pct = ((r - k) / k * 100.0).abs();
                        result.add_mismatch(*target, *prob, r, k, diff_pct);
                    } else {
                        result.matches += 1;
                    }
                }
                (None, None) => {
                    // Both null is OK
                    result.matches += 1;
                }
                (Some(r), None) => {
                    let prob_pct = prob * 100.0;
                    result.add_error(format!(
                        "Rust has fee for {target}@{prob_pct:.0}% ({r:.2}) but Kotlin doesn't"
                    ));
                }
                (None, Some(k)) => {
                    let prob_pct = prob * 100.0;
                    result.add_error(format!(
                        "Kotlin has fee for {target}@{prob_pct:.0}% ({k:.2}) but Rust doesn't"
                    ));
                }
            }
        }
    }

    result
}

#[derive(Debug, Default)]
pub struct ComparisonResult {
    pub matches: usize,
    pub mismatches: Vec<FeeMismatch>,
    pub errors: Vec<String>,
}

impl ComparisonResult {
    pub fn is_success(&self) -> bool {
        self.mismatches.is_empty() && self.errors.is_empty()
    }

    pub fn add_mismatch(
        &mut self,
        target: u32,
        prob: f64,
        rust_fee: f64,
        kotlin_fee: f64,
        diff_pct: f64,
    ) {
        self.mismatches.push(FeeMismatch {
            target,
            probability: prob,
            rust_fee,
            kotlin_fee,
            difference_pct: diff_pct,
        });
    }

    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
    }

    pub fn print_summary(&self, test_name: &str) {
        use colored::*;

        if self.is_success() {
            let test_name_colored = test_name.green();
            let matches = self.matches;
            println!("  ✅ {test_name_colored}: All {matches} fee rates match");
        } else {
            let test_name_colored = test_name.red();
            let matches = self.matches;
            let mismatches_count = self.mismatches.len();
            let errors_count = self.errors.len();
            println!(
                "  ❌ {test_name_colored}: {matches} matches, {mismatches_count} mismatches, {errors_count} errors"
            );

            // Print mismatches
            for mismatch in &self.mismatches {
                let target = mismatch.target;
                let prob_pct = mismatch.probability * 100.0;
                let rust_fee = mismatch.rust_fee;
                let kotlin_fee = mismatch.kotlin_fee;
                let diff = mismatch.difference_pct;
                println!(
                    "    ⚠️ {target}@{prob_pct:.0}%: Rust={rust_fee:.2}, Kotlin={kotlin_fee:.2} (diff={diff:.2}%)"
                );
            }

            // Print errors
            for error in &self.errors {
                println!("    ❌ {error}");
            }
        }
    }
}

#[derive(Debug)]
pub struct FeeMismatch {
    pub target: u32,
    pub probability: f64,
    pub rust_fee: f64,
    pub kotlin_fee: f64,
    pub difference_pct: f64,
}
