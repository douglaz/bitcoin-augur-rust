# Bitcoin Augur - Rust Implementation

A Rust port of the Bitcoin Augur fee estimation library, providing accurate fee estimates by analyzing historical mempool data using statistical modeling.

## Features

- ✅ Statistical fee estimation based on historical mempool data
- ✅ Multiple confidence levels (5%, 20%, 50%, 80%, 95%)
- ✅ Various block targets (3-144 blocks)
- ✅ Static musl binary compilation for deployment portability
- ✅ Nix flakes development environment
- ✅ Full API compatibility with Kotlin implementation

## Project Structure

This workspace contains two crates:

- **`bitcoin-augur`**: Core library for fee estimation
- **`bitcoin-augur-server`**: HTTP server with REST API (coming soon)

## Development Setup

### Using Nix (Recommended)

This project uses Nix flakes for a reproducible development environment.

```bash
# Enter development shell
nix develop

# Or use direnv (after running 'direnv allow')
cd bitcoin-augur-rust
```

### Building

```bash
# Build in development mode
nix develop -c cargo build

# Build optimized static binary
nix develop -c cargo build --release --target x86_64-unknown-linux-musl

# Run tests
nix develop -c cargo test

# Run benchmarks
nix develop -c cargo bench

# Or enter the Nix shell for multiple commands
nix develop
cargo build
cargo test
```

### Static Binary Verification

```bash
# Build the static binary
nix develop -c cargo build --release --target x86_64-unknown-linux-musl

# Verify it's statically linked
ldd target/x86_64-unknown-linux-musl/release/bitcoin-augur-server
# Should output: "not a dynamic executable"
```

## Usage

### As a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
bitcoin-augur = { git = "https://github.com/block/bitcoin-augur-rust" }
```

Example usage:

```rust
use bitcoin_augur::{FeeEstimator, MempoolSnapshot, MempoolTransaction};
use chrono::Utc;

// Initialize the estimator
let fee_estimator = FeeEstimator::new();

// Create mempool snapshot from transactions
let transactions = vec![
    MempoolTransaction::new(565, 1000),  // weight, fee in sats
    MempoolTransaction::new(400, 800),
];

let snapshot = MempoolSnapshot::from_transactions(
    transactions,
    850000,  // block height
    Utc::now(),
);

// Calculate estimates (normally you'd have 24 hours of snapshots)
let fee_estimate = fee_estimator
    .calculate_estimates(&[snapshot])
    .expect("Failed to calculate estimates");

// Get fee rate for 6 blocks with 95% confidence
if let Some(fee_rate) = fee_estimate.get_fee_rate(6, 0.95) {
    println!("Recommended fee: {:.2} sat/vB", fee_rate);
}
```

## API Compatibility

This implementation maintains full API compatibility with the original Kotlin implementation, including:

- Same fee bucket calculation (logarithmic)
- Same statistical modeling approach
- Same API response format for the server
- Same configuration options

## License

Apache 2.0 - See [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! Please read the [PLAN.md](../PLAN.md) file for implementation details and roadmap.

## Acknowledgments

This is a Rust port of the original [Bitcoin Augur](https://github.com/block/bitcoin-augur) library written in Kotlin.