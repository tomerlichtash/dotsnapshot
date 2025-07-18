# Homebrew Setup Guide

This guide helps you set up dotsnapshot for distribution via Homebrew.

## Overview

The dotsnapshot CLI is prepared for Homebrew distribution with the following features:

- ✅ Cross-platform binary releases (macOS Intel, macOS ARM, Linux x86_64)
- ✅ Compressed archives (.tar.gz) with SHA256 checksums
- ✅ Shell completions for bash, zsh, and fish
- ✅ Man page generation
- ✅ Automated testing via installation script
- ✅ Homebrew formula template

## Prerequisites

Before creating a Homebrew formula, ensure you have:

1. **Released version**: A tagged release in your GitHub repository
2. **Binaries**: Cross-platform binaries built and uploaded to GitHub releases
3. **SHA256 checksums**: Generated for each binary archive

## Steps to Create Homebrew Formula

### 1. Get SHA256 Checksums

After a release is created, download the SHA256 files and note the checksums:

```bash
# Download checksums from GitHub release
curl -L -o checksums.txt https://github.com/tomerlichtash/dotsnapshot/releases/download/v1.1.0/dotsnapshot-macos-arm64.sha256
curl -L -o checksums.txt https://github.com/tomerlichtash/dotsnapshot/releases/download/v1.1.0/dotsnapshot-macos-x86_64.sha256
curl -L -o checksums.txt https://github.com/tomerlichtash/dotsnapshot/releases/download/v1.1.0/dotsnapshot-linux-x86_64.sha256
```

### 2. Update Formula Template

Update the `Formula/dotsnapshot.rb` file with the actual SHA256 checksums:

```ruby
# Replace placeholders with actual checksums
sha256 "REPLACE_WITH_ACTUAL_SHA256_FOR_ARM64"    # macOS ARM64
sha256 "REPLACE_WITH_ACTUAL_SHA256_FOR_X86_64"   # macOS Intel
sha256 "REPLACE_WITH_ACTUAL_SHA256_FOR_LINUX"    # Linux x86_64
```

### 3. Test Formula Locally

```bash
# Test the formula locally
brew install --formula ./Formula/dotsnapshot.rb

# Test the installed binary
dotsnapshot --version
dotsnapshot --info
dotsnapshot --list
dotsnapshot --completions bash | head -5
dotsnapshot --man | head -10
```

### 4. Submit to Homebrew

For official Homebrew distribution:

1. **Fork homebrew-core**: Fork the [homebrew-core](https://github.com/Homebrew/homebrew-core) repository
2. **Create formula**: Add your formula to `Formula/dotsnapshot.rb`
3. **Test thoroughly**: Run `brew test dotsnapshot` and `brew audit dotsnapshot`
4. **Submit PR**: Create a pull request to homebrew-core

### 5. Create Custom Tap (Alternative)

For a custom tap:

```bash
# Create a tap repository
gh repo create homebrew-tools --public

# Add formula to the tap
git clone https://github.com/tomerlichtash/homebrew-tools
cd homebrew-tools
mkdir Formula
cp ../dotsnapshot/Formula/dotsnapshot.rb Formula/
git add Formula/dotsnapshot.rb
git commit -m "Add dotsnapshot formula"
git push origin main
```

Users can then install via:
```bash
brew tap tomerlichtash/tools
brew install dotsnapshot
```

## Testing

The formula includes comprehensive tests:

- Version command verification
- Info command output validation
- Plugin listing functionality
- Help command verification
- Shell completions generation
- Man page generation

## Shell Completions

After installation, shell completions are automatically available:

- **Bash**: `/usr/local/etc/bash_completion.d/dotsnapshot`
- **Zsh**: `/usr/local/share/zsh/site-functions/_dotsnapshot`
- **Fish**: `/usr/local/share/fish/vendor_completions.d/dotsnapshot.fish`

## Man Page

The man page is installed to `/usr/local/share/man/man1/dotsnapshot.1` and accessible via:

```bash
man dotsnapshot
```

## Updating the Formula

For new releases:

1. Update the `version` field in the formula
2. Update the `url` fields with new release URLs
3. Update the `sha256` fields with new checksums
4. Test the updated formula
5. Submit PR or update your tap

## Troubleshooting

### Common Issues

1. **SHA256 mismatch**: Ensure checksums match exactly
2. **URL not found**: Verify release exists and URLs are correct
3. **Build failures**: Check that all dependencies are specified
4. **Test failures**: Ensure all test commands work correctly

### Verification Commands

```bash
# Verify URLs are accessible
curl -I https://github.com/tomerlichtash/dotsnapshot/releases/download/v1.1.0/dotsnapshot-macos-arm64.tar.gz

# Verify SHA256
shasum -a 256 dotsnapshot-macos-arm64.tar.gz

# Test formula syntax
brew audit --formula ./Formula/dotsnapshot.rb
```

## Resources

- [Homebrew Formula Cookbook](https://docs.brew.sh/Formula-Cookbook)
- [Homebrew Acceptable Formulae](https://docs.brew.sh/Acceptable-Formulae)
- [Creating Homebrew Taps](https://docs.brew.sh/How-to-Create-and-Maintain-a-Tap)