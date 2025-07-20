#!/bin/bash
# Test script for semantic-release Cargo.toml version update logic
# This validates the prepareCmd from .releaserc.json works correctly

set -e

echo "🧪 Testing Cargo.toml version update logic..."

# Create a test Cargo.toml
cat > test-Cargo.toml << 'EOF'
[package]
name = "dotsnapshot"
version = "1.2.1"
edition = "2021"
description = "A dotfiles snapshot tool"

[dependencies]
clap = "4.0"
EOF

echo "📄 Original test-Cargo.toml:"
cat test-Cargo.toml
echo ""

# Test the sed command from .releaserc.json
TEST_VERSION="1.3.0"
echo "🔄 Running version update command with version: $TEST_VERSION"

# This is the exact command from .releaserc.json (without cargo update)
sed -i.bak "s/^version = \"[^\"]*\"/version = \"$TEST_VERSION\"/" test-Cargo.toml

echo "✅ Updated test-Cargo.toml:"
cat test-Cargo.toml
echo ""

# Verify the change worked
if grep -q "version = \"$TEST_VERSION\"" test-Cargo.toml; then
    echo "✅ SUCCESS: Version successfully updated to $TEST_VERSION"
else
    echo "❌ FAILED: Version was not updated correctly"
    exit 1
fi

# Test with different version formats
echo ""
echo "🧪 Testing with different version formats..."

# Test patch version
TEST_VERSION="1.3.1"
sed -i.bak "s/^version = \"[^\"]*\"/version = \"$TEST_VERSION\"/" test-Cargo.toml
if grep -q "version = \"$TEST_VERSION\"" test-Cargo.toml; then
    echo "✅ Patch version update works: $TEST_VERSION"
else
    echo "❌ Patch version update failed"
    exit 1
fi

# Test major version
TEST_VERSION="2.0.0"
sed -i.bak "s/^version = \"[^\"]*\"/version = \"$TEST_VERSION\"/" test-Cargo.toml
if grep -q "version = \"$TEST_VERSION\"" test-Cargo.toml; then
    echo "✅ Major version update works: $TEST_VERSION"
else
    echo "❌ Major version update failed"
    exit 1
fi

# Test prerelease version
TEST_VERSION="2.1.0-alpha.1"
sed -i.bak "s/^version = \"[^\"]*\"/version = \"$TEST_VERSION\"/" test-Cargo.toml
if grep -q "version = \"$TEST_VERSION\"" test-Cargo.toml; then
    echo "✅ Prerelease version update works: $TEST_VERSION"
else
    echo "❌ Prerelease version update failed"
    exit 1
fi

echo ""
echo "📋 Final test-Cargo.toml content:"
cat test-Cargo.toml

# Cleanup
rm -f test-Cargo.toml test-Cargo.toml.bak

echo ""
echo "🎉 All tests passed! The version update logic works correctly."
echo "✅ The sed command in .releaserc.json should work properly."