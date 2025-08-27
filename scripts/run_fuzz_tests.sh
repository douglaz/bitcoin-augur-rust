#!/usr/bin/env bash

set -e

# Script to run fuzz tests
# Usage: ./scripts/run_fuzz_tests.sh [target] [duration]

TARGET=${1:-all}
DURATION=${2:-60}  # Default 60 seconds per target

echo "Running fuzz tests for $DURATION seconds per target..."

# List of fuzz targets
TARGETS=(
    "fee_calculation"
    "snapshot_parsing"
    "api_input_validation"
    "rpc_response_parsing"
)

# Function to run a single fuzz target
run_fuzz_target() {
    local target=$1
    echo "Fuzzing $target for $DURATION seconds..."
    
    # Create corpus directory if it doesn't exist
    mkdir -p fuzz/corpus/$target
    
    # Run the fuzzer
    timeout $DURATION cargo fuzz run $target -- -max_total_time=$DURATION || true
    
    echo "Finished fuzzing $target"
    echo "---"
}

# Run specified target or all targets
if [ "$TARGET" == "all" ]; then
    for target in "${TARGETS[@]}"; do
        run_fuzz_target $target
    done
else
    run_fuzz_target $TARGET
fi

echo "Fuzz testing complete!"