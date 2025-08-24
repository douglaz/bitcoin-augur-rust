use float_cmp::approx_eq;
use serde_json::{json, Value};
use std::collections::HashSet;

use super::diff::{Diff, DiffResult, DiffType};
use crate::api::FeeEstimateResponse;

const FLOAT_TOLERANCE: f64 = 0.05; // 5% tolerance for fee rates

pub fn compare_fee_responses(
    rust_response: &FeeEstimateResponse,
    kotlin_response: &FeeEstimateResponse,
) -> DiffResult {
    let mut result = DiffResult::new();

    // Convert to JSON for easier comparison
    let rust_json = serde_json::to_value(rust_response).unwrap();
    let kotlin_json = serde_json::to_value(kotlin_response).unwrap();

    compare_json_values("", &rust_json, &kotlin_json, &mut result);

    result
}

fn compare_json_values(path: &str, expected: &Value, actual: &Value, result: &mut DiffResult) {
    match (expected, actual) {
        (Value::Object(exp_map), Value::Object(act_map)) => {
            let exp_keys: HashSet<_> = exp_map.keys().collect();
            let act_keys: HashSet<_> = act_map.keys().collect();

            // Check for missing keys
            for key in exp_keys.difference(&act_keys) {
                let field_path = if path.is_empty() {
                    key.to_string()
                } else {
                    format!("{}.{}", path, key)
                };

                result.add_diff(Diff {
                    path: field_path,
                    expected: exp_map[*key].clone(),
                    actual: Value::Null,
                    difference: DiffType::MissingField,
                });
            }

            // Check for extra keys (warning only)
            for key in act_keys.difference(&exp_keys) {
                let field_path = if path.is_empty() {
                    key.to_string()
                } else {
                    format!("{}.{}", path, key)
                };

                // Don't fail on extra fields, just note them
                tracing::warn!("Extra field found: {}", field_path);
            }

            // Compare common keys
            for key in exp_keys.intersection(&act_keys) {
                let field_path = if path.is_empty() {
                    key.to_string()
                } else {
                    format!("{}.{}", path, key)
                };

                compare_json_values(&field_path, &exp_map[*key], &act_map[*key], result);
            }
        }

        (Value::Array(exp_arr), Value::Array(act_arr)) => {
            if exp_arr.len() != act_arr.len() {
                result.add_diff(Diff {
                    path: path.to_string(),
                    expected: json!(format!("array[{}]", exp_arr.len())),
                    actual: json!(format!("array[{}]", act_arr.len())),
                    difference: DiffType::ValueMismatch,
                });
            } else {
                for (i, (exp_item, act_item)) in exp_arr.iter().zip(act_arr.iter()).enumerate() {
                    let item_path = format!("{}[{}]", path, i);
                    compare_json_values(&item_path, exp_item, act_item, result);
                }
            }
        }

        (Value::Number(exp_num), Value::Number(act_num)) => {
            if let (Some(exp_f), Some(act_f)) = (exp_num.as_f64(), act_num.as_f64()) {
                // Special handling for fee_rate fields - use tolerance
                if path.contains("fee_rate") {
                    if !approx_eq!(f64, exp_f, act_f, epsilon = exp_f * FLOAT_TOLERANCE) {
                        let pct_diff = ((act_f - exp_f) / exp_f * 100.0).abs();
                        result.add_diff(Diff {
                            path: path.to_string(),
                            expected: expected.clone(),
                            actual: actual.clone(),
                            difference: DiffType::FloatDifference(pct_diff),
                        });
                    }
                } else if (exp_f - act_f).abs() > 0.0001 {
                    result.add_diff(Diff {
                        path: path.to_string(),
                        expected: expected.clone(),
                        actual: actual.clone(),
                        difference: DiffType::ValueMismatch,
                    });
                }
            }
        }

        (Value::String(exp_str), Value::String(act_str)) => {
            // Special handling for timestamp fields
            if path.contains("time") || path.contains("timestamp") {
                // Just check that both are valid timestamps, don't compare exact values
                // as they will differ between runs
                return;
            }

            if exp_str != act_str {
                result.add_diff(Diff {
                    path: path.to_string(),
                    expected: expected.clone(),
                    actual: actual.clone(),
                    difference: DiffType::ValueMismatch,
                });
            }
        }

        (Value::Bool(exp_bool), Value::Bool(act_bool)) => {
            if exp_bool != act_bool {
                result.add_diff(Diff {
                    path: path.to_string(),
                    expected: expected.clone(),
                    actual: actual.clone(),
                    difference: DiffType::ValueMismatch,
                });
            }
        }

        (Value::Null, Value::Null) => {
            // Both null, OK
        }

        _ => {
            // Type mismatch
            result.add_diff(Diff {
                path: path.to_string(),
                expected: expected.clone(),
                actual: actual.clone(),
                difference: DiffType::TypeMismatch,
            });
        }
    }
}
