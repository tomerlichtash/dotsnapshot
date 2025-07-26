#!/bin/bash

# Test script with mandatory 90% coverage threshold
# This script ensures local development maintains high code quality standards

set -e

COVERAGE_THRESHOLD=90

echo "🧪 Running tests with coverage analysis..."
echo "📊 Required coverage threshold: ${COVERAGE_THRESHOLD}%"
echo ""

# Check if cargo-llvm-cov is installed
if ! command -v cargo-llvm-cov &> /dev/null; then
    echo "❌ cargo-llvm-cov is not installed"
    echo "Installing cargo-llvm-cov..."
    cargo install cargo-llvm-cov
fi

# Check if llvm-tools is installed (not llvm-tools-preview)
if ! rustup component list --installed | grep -q llvm-tools; then
    echo "❌ llvm-tools is not installed"
    echo "Installing llvm-tools..."
    rustup component add llvm-tools
fi

echo "🔍 Running comprehensive test suite with coverage..."
echo ""

# Set up LLVM tools paths if not already set
if [[ -z "$LLVM_COV" || -z "$LLVM_PROFDATA" ]]; then
    # First try homebrew LLVM installation
    if [[ -f "/opt/homebrew/Cellar/llvm/20.1.8/bin/llvm-cov" ]]; then
        export LLVM_COV="/opt/homebrew/Cellar/llvm/20.1.8/bin/llvm-cov"
        export LLVM_PROFDATA="/opt/homebrew/Cellar/llvm/20.1.8/bin/llvm-profdata"
        echo "📍 Using Homebrew LLVM tools at: /opt/homebrew/Cellar/llvm/20.1.8/bin/"
    else
        # Fallback to rustup toolchain LLVM tools
        TOOLCHAIN_PATH=$(rustup toolchain list | grep default | awk '{print $1}' | head -1)
        if [[ -z "$TOOLCHAIN_PATH" ]]; then
            TOOLCHAIN_PATH="stable-$(rustc -vV | grep host | cut -d' ' -f2)"
        fi
        
        LLVM_TOOLS_DIR="$HOME/.rustup/toolchains/$TOOLCHAIN_PATH/lib/rustlib/$(rustc -vV | grep host | cut -d' ' -f2)/bin"
        
        if [[ -f "$LLVM_TOOLS_DIR/llvm-cov" ]]; then
            export LLVM_COV="$LLVM_TOOLS_DIR/llvm-cov"
            export LLVM_PROFDATA="$LLVM_TOOLS_DIR/llvm-profdata"
            echo "📍 Found LLVM tools at: $LLVM_TOOLS_DIR"
        else
            echo "❌ Could not find LLVM tools. Please install with:"
            echo "   brew install llvm"
            echo "   - or -"
            echo "   rustup component add llvm-tools-preview"
            exit 1
        fi
    fi
fi

# Run tests with coverage
cargo llvm-cov --lib --all-features --lcov --output-path lcov.info --color always

echo ""
echo "📈 Coverage Analysis Results:"
echo "================================"

# Get coverage summary
COVERAGE_OUTPUT=$(cargo llvm-cov --lib --all-features --summary-only --color always)
echo "$COVERAGE_OUTPUT"

# Extract total coverage percentage
COVERAGE=$(echo "$COVERAGE_OUTPUT" | grep "TOTAL" | awk '{print $4}' | sed 's/%//')

echo ""
echo "📊 Total Coverage: ${COVERAGE}%"
echo "🎯 Required Threshold: ${COVERAGE_THRESHOLD}%"

# Check if coverage meets threshold
if (( $(echo "$COVERAGE < $COVERAGE_THRESHOLD" | bc -l) )); then
    echo ""
    echo "❌ COVERAGE CHECK FAILED"
    echo "   Current coverage: ${COVERAGE}%"
    echo "   Required coverage: ${COVERAGE_THRESHOLD}%"
    echo "   Shortfall: $(echo "$COVERAGE_THRESHOLD - $COVERAGE" | bc -l)%"
    echo ""
    echo "🛠️  Action Required:"
    echo "   - Add tests for uncovered code paths"
    echo "   - Focus on files with low coverage (see detailed report above)"
    echo "   - Consider integration tests for core functionality"
    echo ""
    echo "💡 Generate detailed HTML report with:"
    echo "   cargo llvm-cov --all-features --workspace --html"
    echo ""
    exit 1
else
    echo ""
    echo "✅ COVERAGE CHECK PASSED"
    echo "   Coverage ${COVERAGE}% meets the ${COVERAGE_THRESHOLD}% threshold"
    echo ""
    echo "🎉 All tests passed with sufficient coverage!"
    echo ""
fi

# Generate HTML report for detailed analysis
echo "📄 Generating detailed HTML coverage report..."
cargo llvm-cov --lib --all-features --html --output-dir coverage-report --color always

echo "🔗 View detailed coverage report:"
echo "   open coverage-report/index.html"
echo ""
echo "✅ Test suite completed successfully!"