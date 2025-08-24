# Bitcoin Augur Integration Tests

A comprehensive integration test suite that compares the Rust and Kotlin/Java implementations of Bitcoin Augur to ensure behavioral parity.

## Features

- 🔄 Runs both Rust and Kotlin servers simultaneously
- 📊 Compares API responses for consistency
- ✅ Validates fee estimation accuracy
- 🚀 Performance benchmarking
- 📈 Detailed reporting with colored output

## Prerequisites

### For Rust Server
- Build the Rust server: `cargo build --release -p bitcoin-augur-server`

### For Kotlin Server
- Java 11 or later
- Build the reference implementation:
  ```bash
  cd ../bitcoin-augur-reference
  gradle build
  ```

### Bitcoin Core (Optional)
- Running Bitcoin Core with RPC enabled
- Configure RPC credentials

## Installation

```bash
# Build the integration test binary
cargo build --release -p bitcoin-augur-integration-tests
```

## Usage

### Validate Environment

Check that all prerequisites are met:

```bash
cargo run -p bitcoin-augur-integration-tests -- validate
```

### Run Full Test Suite

Compare both implementations:

```bash
cargo run -p bitcoin-augur-integration-tests -- test \
  --rust-port 8080 \
  --kotlin-port 8081 \
  --bitcoin-rpc http://localhost:8332 \
  --rpc-user myuser \
  --rpc-password mypass
```

### Test Single Server

Test only the Rust implementation:

```bash
cargo run -p bitcoin-augur-integration-tests -- test \
  --skip-kotlin \
  --rust-port 8080
```

Test only the Kotlin implementation:

```bash
cargo run -p bitcoin-augur-integration-tests -- test \
  --skip-rust \
  --kotlin-port 8081
```

### Advanced Options

```bash
# Specify custom binary/JAR paths
cargo run -p bitcoin-augur-integration-tests -- test \
  --rust-binary /path/to/bitcoin-augur-server \
  --kotlin-jar /path/to/app.jar

# Increase startup timeout (default 30s)
cargo run -p bitcoin-augur-integration-tests -- test \
  --startup-timeout 60

# Output results as JSON
cargo run -p bitcoin-augur-integration-tests -- test --json

# Enable verbose logging
cargo run -p bitcoin-augur-integration-tests -- test --verbose
```

## Test Scenarios

### Basic Tests
- ✅ `/fees` endpoint comparison
- ✅ `/fees/target/{blocks}` for various block targets (3, 6, 12, 24, 144)
- ✅ Response structure validation
- ✅ Health check endpoints

### Advanced Tests
- ⚡ Performance comparison (average response time)
- 🔄 Concurrent request handling
- 📊 Response structure validation
- 🎯 Fee rate accuracy (within 5% tolerance)

## Output

The test suite provides colored terminal output with:
- ✅ Green for passed tests
- ❌ Red for failed tests
- ⚠️ Yellow for warnings or skipped tests
- Detailed diff output for mismatches
- Summary statistics

Example output:
```
Bitcoin Augur Integration Tests
================================
✅ Rust server started successfully
✅ Kotlin server started successfully

Running Basic Comparison Tests
-------------------------------
📊 Test: Compare /fees endpoint
  ✅ Responses match

📊 Test: Compare /fees/target/6 endpoint
  ✅ 6 blocks: Responses match

Running Advanced Tests
----------------------
⚡ Test: Performance comparison
  📊 Rust server:   25 ms average
  📊 Kotlin server: 30 ms average
  ✅ Both servers respond quickly (Kotlin is 1.2x faster)

════════════════════════════════════════════════════════════
Test Summary
════════════════════════════════════════════════════════════
Server Status:
  ✅ Rust server started successfully
  ✅ Kotlin server started successfully

Test Results:
  Total:   8
  Passed:  8 ✅
  Failed:  0 ❌
  Skipped: 0 ⚠️

✅ All tests passed!
════════════════════════════════════════════════════════════
```

## Configuration

Both servers are configured with temporary directories and identical settings:
- Same mempool collection interval (30s)
- Same Bitcoin RPC connection
- Same data retention policy
- Isolated data directories (cleaned up after tests)

## Troubleshooting

### Rust Server Won't Start
- Ensure the binary is built: `cargo build --release -p bitcoin-augur-server`
- Check the port is not in use
- Verify Bitcoin RPC credentials if provided

### Kotlin Server Won't Start
- Ensure Java is installed: `java -version`
- Build the JAR: `cd ../bitcoin-augur-reference && gradle build`
- Check the port is not in use

### No Fee Data Available
- Servers need time to collect mempool data
- Default wait time is 5 seconds after startup
- Ensure Bitcoin Core is running and accessible

## Development

### Project Structure
```
bitcoin-augur-integration-tests/
├── src/
│   ├── main.rs          # Entry point
│   ├── cli.rs           # CLI argument parsing
│   ├── server/          # Server process management
│   ├── api/             # API client and models
│   ├── comparison/      # Response comparison logic
│   ├── tests/           # Test scenarios
│   └── report/          # Test reporting
```

### Adding New Tests

1. Add test logic to `src/tests/basic.rs` or `src/tests/advanced.rs`
2. Update the test report with new test names
3. Follow the existing pattern for comparison tests

### Tolerance Settings

- Fee rates: 5% tolerance (configurable in `fee_compare.rs`)
- Timestamps: Ignored in comparisons
- Extra fields: Logged as warnings but don't fail tests