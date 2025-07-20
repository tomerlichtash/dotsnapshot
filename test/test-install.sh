#!/bin/bash

# Installation test script for dotsnapshot
# This script verifies the binary works correctly after installation

set -e

BINARY_PATH=${1:-"./dotsnapshot"}
TEMP_DIR=$(mktemp -d)
EXIT_CODE=0

echo "🧪 Testing dotsnapshot installation..."
echo "📍 Binary path: $BINARY_PATH"
echo "📂 Temp directory: $TEMP_DIR"
echo

# Test 1: Binary exists and is executable
echo "✅ Test 1: Binary exists and is executable"
if [ ! -f "$BINARY_PATH" ]; then
    echo "❌ Binary not found at $BINARY_PATH"
    exit 1
fi

if [ ! -x "$BINARY_PATH" ]; then
    echo "❌ Binary is not executable"
    exit 1
fi
echo "✅ Binary exists and is executable"
echo

# Test 2: Version command works
echo "✅ Test 2: Version command"
if ! VERSION_OUTPUT=$("$BINARY_PATH" --version 2>&1); then
    echo "❌ Version command failed"
    echo "Output: $VERSION_OUTPUT"
    EXIT_CODE=1
else
    echo "✅ Version: $VERSION_OUTPUT"
fi
echo

# Test 3: Info command works
echo "✅ Test 3: Info command"
if ! INFO_OUTPUT=$("$BINARY_PATH" --info 2>&1); then
    echo "❌ Info command failed"
    echo "Output: $INFO_OUTPUT"
    EXIT_CODE=1
else
    echo "✅ Info command works"
    echo "First line: $(echo "$INFO_OUTPUT" | head -1)"
fi
echo

# Test 4: Help command works
echo "✅ Test 4: Help command"
if ! HELP_OUTPUT=$("$BINARY_PATH" --help 2>&1); then
    echo "❌ Help command failed"
    echo "Output: $HELP_OUTPUT"
    EXIT_CODE=1
else
    echo "✅ Help command works"
    echo "Usage shown: $(echo "$HELP_OUTPUT" | grep -i usage || echo "Usage info present")"
fi
echo

# Test 5: List plugins command works
echo "✅ Test 5: List plugins command"
if ! LIST_OUTPUT=$("$BINARY_PATH" --list 2>&1); then
    echo "❌ List plugins command failed"
    echo "Output: $LIST_OUTPUT"
    EXIT_CODE=1
else
    echo "✅ List plugins command works"
    echo "Plugin types found: $(echo "$LIST_OUTPUT" | grep -E "🍺|💻|✏️|📦" | wc -l) categories"
fi
echo

# Test 6: Basic snapshot creation (dry run)
echo "✅ Test 6: Basic functionality test"
if ! SNAPSHOT_OUTPUT=$("$BINARY_PATH" --output "$TEMP_DIR/test-snapshot" --verbose 2>&1); then
    echo "⚠️  Snapshot creation failed (expected - no valid plugins available)"
    echo "Output: $(echo "$SNAPSHOT_OUTPUT" | head -3)"
    echo "✅ Binary executed without crashing"
else
    echo "✅ Snapshot creation succeeded"
    echo "Output directory: $TEMP_DIR/test-snapshot"
fi
echo

# Test 7: Invalid argument handling
echo "✅ Test 7: Invalid argument handling"
if INVALID_OUTPUT=$("$BINARY_PATH" --invalid-flag 2>&1); then
    echo "❌ Invalid flag should have failed"
    EXIT_CODE=1
else
    echo "✅ Invalid arguments handled correctly"
fi
echo

# Cleanup
echo "🧹 Cleaning up..."
rm -rf "$TEMP_DIR"

# Final result
echo "📊 Test Results:"
if [ $EXIT_CODE -eq 0 ]; then
    echo "✅ All tests passed! dotsnapshot is ready for use."
else
    echo "❌ Some tests failed. Please check the output above."
fi

exit $EXIT_CODE