# Semantic Release Workflow

## Overview

This project uses **semantic-release** for fully automated releases based on conventional commit messages. No manual version management is required.

## Current Configuration

### Semantic Release Setup
- **Configuration**: `.releaserc.jsonno`
- **Workflow**: `.github/workflows/release.yml`
- **Trigger**: Every push to `main` branch (via merged PRs)
- **Versioning**: Automatic based on commit message analysis

### Supported Branches
- `main` - Production releases
- `next` - Pre-release testing
- `beta` - Beta releases  
- `alpha` - Alpha releases

## Release Process

### 1. Development Workflow
```bash
# Create feature branch
git checkout -b feat/new-feature

# Make changes with semantic commits
git commit -m "feat: Add new snapshot filtering capability"
git commit -m "fix: Resolve path resolution issue on Windows"

# Create PR to main
gh pr create --title "feat: Add snapshot filtering" --body "..."
```

### 2. Automatic Release
When PR is merged to `main`:
1. **Semantic-release analyzes** commit messages since last release
2. **Determines version bump**:
   - `feat:` → Minor version (1.2.0 → 1.3.0)
   - `fix:` → Patch version (1.2.0 → 1.2.1)
   - `BREAKING CHANGE:` → Major version (1.2.0 → 2.0.0)
3. **Updates files**:
   - Cargo.toml version
   - Cargo.lock dependencies
   - CHANGELOG.md generation
4. **Builds binaries** for all platforms:
   - Linux x86_64 and ARM64
   - macOS x86_64 and ARM64 (Apple Silicon)
   - Windows x86_64 (temporarily disabled during release refactoring)
5. **Creates GitHub release** with:
   - Git tag (e.g., `v1.3.0`)
   - Release notes from commits
   - Binary assets and checksums
6. **Commits changes** back to main with `[skip ci]`

### 3. Manual Release (Emergency)
For urgent releases without qualifying commits:
```bash
# Via GitHub Actions UI:
# 1. Go to Actions → Release workflow
# 2. Click "Run workflow" 
# 3. Select branch (usually main)
# 4. Click "Run workflow"
```

## Commit Message Format

### Required Format
```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

### Commit Types
- **feat**: New feature (minor version bump)
- **fix**: Bug fix (patch version bump)
- **docs**: Documentation changes (patch version bump)
- **style**: Code style changes (patch version bump)
- **refactor**: Code refactoring (patch version bump)
- **test**: Adding or updating tests (patch version bump)
- **chore**: Maintenance tasks (patch version bump)
- **ci**: CI/CD changes (patch version bump)

### Breaking Changes
```bash
# Method 1: Footer
git commit -m "feat: New API endpoint

BREAKING CHANGE: Old endpoint /api/v1 removed"

# Method 2: Exclamation mark
git commit -m "feat!: Redesign configuration format"
```

### Examples
```bash
✅ git commit -m "feat: Add GitHub Pages documentation site"
✅ git commit -m "fix: Resolve Cargo.lock version conflicts"
✅ git commit -m "docs: Update installation instructions"
✅ git commit -m "feat!: Change config file format to TOML"

❌ git commit -m "Add new feature"  # No semantic prefix
❌ git commit -m "fix: resolve bug"  # Not capitalized
❌ git commit -m "WIP: working on feature"  # Not semantic
```

## Version Management

### Automatic Version Updates
Semantic-release handles all version management:
- **Cargo.toml**: Version field updated automatically
- **Cargo.lock**: Dependencies updated with `cargo update`
- **Git tags**: Created automatically (e.g., `v1.3.0`)
- **Binary version**: Matches Cargo.toml via build process

### Version Calculation
```
Current: v1.2.1

Commits since last release:
- fix: Bug fix          → v1.2.2 (patch)
- feat: New feature     → v1.3.0 (minor) 
- feat!: Breaking change → v2.0.0 (major)
```

## Release Assets

### Automatically Built Binaries
- `dotsnapshot-v{version}-x86_64-unknown-linux-gnu.tar.gz`
- `dotsnapshot-v{version}-aarch64-unknown-linux-gnu.tar.gz`
- `dotsnapshot-v{version}-x86_64-apple-darwin.tar.gz`
- `dotsnapshot-v{version}-aarch64-apple-darwin.tar.gz`
- `dotsnapshot-v{version}-x86_64-pc-windows-msvc.zip` (temporarily disabled)

### Checksums
- SHA256 checksums generated for all binaries
- Attached to GitHub release automatically

## Troubleshooting

### No Release Created
**Problem**: PR merged but no release triggered
**Solutions**:
- Check commit messages use semantic prefixes
- Verify commits since last release qualify for version bump
- Check GitHub Actions logs for errors

### Version Mismatch
**Problem**: Binary reports wrong version
**Solutions**:
- Verify Cargo.toml updated correctly
- Check build process includes version from Cargo.toml
- Ensure no cached builds

### Failed Release
**Problem**: Release workflow fails
**Solutions**:
- Check GitHub Actions logs
- Verify all tests pass
- Ensure binary builds successfully
- Check semantic-release configuration

## Migration from Manual Process

### Benefits of Semantic Release
- ✅ **No manual version management**
- ✅ **Consistent commit message format**
- ✅ **Automatic changelog generation**
- ✅ **Multi-platform binary builds**
- ✅ **Version alignment guaranteed**
- ✅ **No human error in releases**

### What Changed
- **Before**: Manual version bumps, manual release creation
- **After**: Commit-message-driven automation
- **Before**: Branch-based release workflow
- **After**: Main-branch continuous delivery

## Best Practices

1. **Write clear commit messages** following conventional format
2. **Use meaningful commit types** that reflect actual changes
3. **Include breaking change indicators** when API changes
4. **Test locally** before creating PRs
5. **Review PR titles** to ensure semantic format
6. **Monitor releases** to verify successful deployment

---

**Result**: Fully automated, predictable, and error-free releases with zero manual intervention required.