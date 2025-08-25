use colored::*;
use serde_json::Value;

#[derive(Debug)]
pub struct Diff {
    pub path: String,
    pub expected: Value,
    pub actual: Value,
    pub difference: DiffType,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum DiffType {
    MissingField,
    ExtraField,
    TypeMismatch,
    ValueMismatch,
    FloatDifference(f64), // percentage difference
}

#[derive(Debug, Default)]
pub struct DiffResult {
    pub diffs: Vec<Diff>,
    pub passed: bool,
}

impl DiffResult {
    pub fn new() -> Self {
        Self {
            diffs: Vec::new(),
            passed: true,
        }
    }

    pub fn add_diff(&mut self, diff: Diff) {
        self.passed = false;
        self.diffs.push(diff);
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.diffs.is_empty()
    }

    pub fn print_summary(&self, name: &str) {
        if self.passed {
            println!("✅ {} {}", name, "PASSED".green().bold());
        } else {
            println!(
                "❌ {} {} with {} differences",
                name,
                "FAILED".red().bold(),
                self.diffs.len()
            );

            for diff in &self.diffs {
                self.print_diff(diff);
            }
        }
    }

    fn print_diff(&self, diff: &Diff) {
        println!("  {}: {}", "Path".yellow(), diff.path);

        match &diff.difference {
            DiffType::MissingField => {
                println!("    {} Missing field", "•".red());
                println!("      Expected: {}", format!("{}", diff.expected).green());
            }
            DiffType::ExtraField => {
                println!("    {} Extra field", "•".yellow());
                println!("      Actual: {}", format!("{}", diff.actual).yellow());
            }
            DiffType::TypeMismatch => {
                println!("    {} Type mismatch", "•".red());
                println!(
                    "      Expected type: {}",
                    value_type(&diff.expected).green()
                );
                println!("      Actual type: {}", value_type(&diff.actual).red());
            }
            DiffType::ValueMismatch => {
                println!("    {} Value mismatch", "•".red());
                println!("      Expected: {}", format!("{}", diff.expected).green());
                println!("      Actual: {}", format!("{}", diff.actual).red());
            }
            DiffType::FloatDifference(pct) => {
                println!("    {} Float difference: {:.2}%", "•".yellow(), pct);
                println!("      Expected: {}", format!("{}", diff.expected).green());
                println!("      Actual: {}", format!("{}", diff.actual).yellow());
            }
        }
    }
}

fn value_type(value: &Value) -> &str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
