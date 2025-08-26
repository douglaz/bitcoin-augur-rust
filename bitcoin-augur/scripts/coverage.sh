#!/usr/bin/env bash
set -euo pipefail

# Script to generate code coverage reports locally

echo "🧪 Generating code coverage report..."
echo ""

# Check if we're in nix develop environment
if [ -z "${IN_NIX_SHELL:-}" ]; then
    echo "⚠️  Not in nix develop environment. Running with nix develop..."
    nix develop -c "$0" "$@"
    exit $?
fi

# Parse command line arguments
HTML_REPORT=false
OPEN_REPORT=false
MIN_COVERAGE=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --html)
            HTML_REPORT=true
            shift
            ;;
        --open)
            OPEN_REPORT=true
            HTML_REPORT=true
            shift
            ;;
        --min)
            MIN_COVERAGE="$2"
            shift 2
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --html        Generate HTML report"
            echo "  --open        Generate and open HTML report in browser"
            echo "  --min <PCT>   Fail if coverage is below PCT percent"
            echo "  --help        Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Run '$0 --help' for usage information"
            exit 1
            ;;
    esac
done

# Build output format argument
OUTPUT_FORMATS="Lcov,Xml"
if [ "$HTML_REPORT" = true ]; then
    OUTPUT_FORMATS="$OUTPUT_FORMATS,Html"
fi

# Build tarpaulin command
TARPAULIN_CMD="cargo tarpaulin"
TARPAULIN_CMD="$TARPAULIN_CMD --out $OUTPUT_FORMATS"
TARPAULIN_CMD="$TARPAULIN_CMD --output-dir target/coverage"
TARPAULIN_CMD="$TARPAULIN_CMD --exclude-files '*/tests/*' '*/benches/*' '*/build.rs'"
TARPAULIN_CMD="$TARPAULIN_CMD --exclude bitcoin-augur-integration-tests"
TARPAULIN_CMD="$TARPAULIN_CMD --timeout 120"
TARPAULIN_CMD="$TARPAULIN_CMD --skip-clean"
TARPAULIN_CMD="$TARPAULIN_CMD --print-summary"

# Add minimum coverage if specified
if [ -n "$MIN_COVERAGE" ]; then
    TARPAULIN_CMD="$TARPAULIN_CMD --fail-under $MIN_COVERAGE"
fi

# Run coverage
echo "Running: $TARPAULIN_CMD"
echo ""
eval $TARPAULIN_CMD

# Get coverage percentage from the XML report
if [ -f "target/coverage/cobertura.xml" ]; then
    COVERAGE=$(grep -oP 'line-rate="\K[^"]+' target/coverage/cobertura.xml | head -1)
    COVERAGE_PCT=$(echo "$COVERAGE * 100" | bc -l | cut -d. -f1)
    
    echo ""
    echo "📊 Overall coverage: ${COVERAGE_PCT}%"
fi

# Open HTML report if requested
if [ "$OPEN_REPORT" = true ] && [ -f "target/coverage/tarpaulin-report.html" ]; then
    echo "📖 Opening coverage report in browser..."
    xdg-open "target/coverage/tarpaulin-report.html" 2>/dev/null || \
        open "target/coverage/tarpaulin-report.html" 2>/dev/null || \
        echo "   Please open target/coverage/tarpaulin-report.html manually"
fi

echo ""
echo "✅ Coverage report generated in target/coverage/"
echo ""
echo "Available reports:"
[ -f "target/coverage/lcov.info" ] && echo "  • LCOV: target/coverage/lcov.info"
[ -f "target/coverage/cobertura.xml" ] && echo "  • XML: target/coverage/cobertura.xml"
[ -f "target/coverage/tarpaulin-report.html" ] && echo "  • HTML: target/coverage/tarpaulin-report.html"