#!/bin/bash
# Test the exact prepareCmd from .releaserc.json with variable substitution
# This simulates how semantic-release will execute the command

set -e

echo "🧪 Testing exact .releaserc.json prepareCmd with variable substitution..."

# Create a test Cargo.toml that matches our actual format
cat > test-Cargo.toml << 'EOF'
[package]
name = "dotsnapshot"
version = "1.2.1"
edition = "2021"
description = "A CLI tool for creating snapshots of dotfiles and configuration directories"
license = "MIT"
repository = "https://github.com/tomerlichtash/dotsnapshot"
authors = ["Tomer Lichtash <tomerlichtash@gmail.com>"]

[dependencies]
clap = { version = "4.4", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
walkdir = "2.3"
sha2 = "0.10"
chrono = { version = "0.4", features = ["serde"] }
EOF

echo "📄 Original test-Cargo.toml:"
cat test-Cargo.toml
echo ""

# Simulate semantic-release variable substitution
export nextRelease_version="1.3.0"

echo "🔄 Testing with nextRelease.version = $nextRelease_version"

# Test the exact command from .releaserc.json (without cargo update to avoid dependency issues)
# Using environment variable substitution like semantic-release does
PREPARE_CMD="sed -i.bak 's/^version = \"[^\"]*\"/version = \"${nextRelease_version}\"/' test-Cargo.toml"

echo "📋 Executing: $PREPARE_CMD"
eval "$PREPARE_CMD"

echo "✅ Updated test-Cargo.toml:"
cat test-Cargo.toml
echo ""

# Verify the change worked
if grep -q "version = \"$nextRelease_version\"" test-Cargo.toml; then
    echo "✅ SUCCESS: Version successfully updated to $nextRelease_version"
else
    echo "❌ FAILED: Version was not updated correctly"
    cat test-Cargo.toml
    exit 1
fi

# Test edge cases
echo ""
echo "🧪 Testing edge cases..."

# Test with prerelease version
export nextRelease_version="2.0.0-beta.1"
PREPARE_CMD="sed -i.bak 's/^version = \"[^\"]*\"/version = \"${nextRelease_version}\"/' test-Cargo.toml"
eval "$PREPARE_CMD"

if grep -q "version = \"$nextRelease_version\"" test-Cargo.toml; then
    echo "✅ Prerelease version works: $nextRelease_version"
else
    echo "❌ Prerelease version failed"
    exit 1
fi

# Verify the rest of the file is unchanged
if grep -q "name = \"dotsnapshot\"" test-Cargo.toml && grep -q "edition = \"2021\"" test-Cargo.toml; then
    echo "✅ Other fields preserved correctly"
else
    echo "❌ Other fields were corrupted"
    exit 1
fi

# Cleanup
rm -f test-Cargo.toml test-Cargo.toml.bak

echo ""
echo "🎉 All .releaserc.json prepareCmd tests passed!"
echo "✅ The command should work correctly in semantic-release."