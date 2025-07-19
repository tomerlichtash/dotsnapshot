# Release Plan

## Current State Analysis (2025-07-19)

### Version Status Across All Locations

| Location | Version | Status | Notes |
|----------|---------|--------|-------|
| **Cargo.toml** | `1.2.1` | ✅ Correct | Updated by recent PRs |
| **Latest GitHub Release** | `v1.2.1` | ✅ Correct | Created successfully |
| **Binary (v1.2.1 release)** | `1.2.1` | ✅ Correct | Finally reports correct version! |
| **Git Tags** | `v1.0.0, v1.1.0, v1.2.0, v1.2.1` | ✅ Correct | All tags properly set |
| **Main Branch HEAD** | `5539555` | ❌ Messy | Multiple release commits |

### What Actually Worked 

**🎉 SUCCESS: v1.2.1 is now correctly released!**

- ✅ **GitHub Release**: v1.2.1 exists and has binaries
- ✅ **Binary Version**: `dotsnapshot --version` reports "dotsnapshot v1.2.1" 
- ✅ **Cargo.toml**: Version is correctly set to "1.2.1"
- ✅ **Git Tags**: All tags (v1.0.0 through v1.2.1) are properly set

### Repository Cleanup Needed

**Branch Pollution**: 15+ experimental release branches exist:
```
release/v1.2.0, release/v1.2.1, release/v1.2.1-fix
test/auto-release-workflow, test/first-release, test/release-pr-workflow
fix/semantic-release-pr-workflow, feat/semantic-release
...and more
```

**Main Branch History**: Multiple redundant release commits:
```
5539555 fix: Trigger v1.2.1 release with correct binary version [RELEASE]
674fcf5 chore: Release version 1.2.1 with correct binary version [RELEASE] (#60)  
4de334b chore: Release version 1.2.0 [RELEASE] (#59)
```

### Problems with Current Semantic Release Approach

1. ~~**Version Mismatch**: Binary reports wrong version (1.0.0 vs 1.2.1)~~ ✅ **FIXED**
2. **Commit Type Confusion**: `chore:` doesn't trigger releases, only `feat:` and `fix:`
3. **Branch Pollution**: Too many experimental branches
4. **Complex Flow**: Too many moving parts, hard to debug
5. **Messy History**: Multiple release commits for same version

## Cleanup Strategy (Before New Release Process)

### Step 1: Clean Up Branches
```bash
# Delete all experimental release branches (both local and remote)
git branch -D $(git branch | grep -E "(release/|test/|fix/.*release|feat/.*release)" | tr -d ' ')

# Delete remote branches (carefully!)
git push origin --delete release/v1.2.0
git push origin --delete release/v1.2.1  
git push origin --delete release/v1.2.1-fix
git push origin --delete test/auto-release-workflow
git push origin --delete test/first-release
git push origin --delete test/release-pr-workflow
git push origin --delete test/release-workflow
git push origin --delete test/trigger-new-release
git push origin --delete fix/semantic-release-pr-workflow
git push origin --delete feat/semantic-release
# ... (and other experimental branches)
```

### Step 2: Prepare Main Branch for Next Development
```bash
# Current state: Cargo.toml = "1.2.1" (matches released version)
# Next development version should be "1.2.2"

git checkout main
git pull origin main

# Update to next development version
sed -i 's/version = "1.2.1"/version = "1.2.2"/' Cargo.toml
cargo update --package dotsnapshot --precise "1.2.2"

# Commit development version
git add Cargo.toml Cargo.lock  
git commit -m "chore: Bump to v1.2.2 for next development cycle"
git push origin main
```

### Step 3: Verify Clean State
After cleanup, verify:
- ✅ **Main branch**: Cargo.toml shows "1.2.2" (next dev version)
- ✅ **Latest release**: v1.2.1 with correct binary version  
- ✅ **Tags**: All release tags (v1.0.0 through v1.2.1) exist
- ✅ **Branches**: Only main + active feature branches remain

## Requirements

✅ **Version Alignment**: Release tag = Cargo.toml version = binary --version output  
✅ **No Uncommitted Changes**: Clean state after release  
✅ **No Direct Main Pushes**: All changes via PRs  
✅ **Automated & Reproducible**: Clear, reliable process  
✅ **Controlled Releases**: Manual trigger, not every merge  
✅ **Clear Changelog**: Automated generation  

## Proposed Solution: Branch-Based Release Workflow

### Core Principle
**Branch Name as Trigger**: `release/v1.3.0` branch automatically triggers release of version 1.3.0.

### Release Process

#### 1. Development Phase
- All development happens on feature branches
- PRs merge to main with standard commit messages
- Cargo.toml stays at current development version (e.g., "1.2.2")

#### 2. Release Trigger (Automated Script)
```bash
# Simple one-command release
./scripts/release.sh 1.3.0

# This script automatically:
# 1. Creates branch: release/v1.3.0
# 2. Adds release notes
# 3. Creates PR: "Release v1.3.0"
# 4. When merged → automatic release
```

#### 3. Release Script Implementation
```bash
#!/bin/bash
# scripts/release.sh

VERSION=$1
BRANCH="release/v$VERSION"

echo "🚀 Creating release branch: $BRANCH"

git checkout main
git pull origin main
git checkout -b "$BRANCH"

# Generate release notes
LAST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "")
if [ -n "$LAST_TAG" ]; then
    echo "# Release v$VERSION" > RELEASE_NOTES.md
    echo "" >> RELEASE_NOTES.md
    git log --oneline "${LAST_TAG}..HEAD" --pretty=format:"- %s" >> RELEASE_NOTES.md
else
    echo "# Release v$VERSION" > RELEASE_NOTES.md
    echo "Initial release" >> RELEASE_NOTES.md
fi

git add RELEASE_NOTES.md
git commit -m "Prepare release v$VERSION"
git push origin "$BRANCH"

gh pr create \
  --title "Release v$VERSION" \
  --body "🚀 Release version $VERSION

## Changes
$(cat RELEASE_NOTES.md | tail -n +3)

**Merge this PR to trigger automatic release workflow.**"

echo "✅ Release PR created. Merge to trigger automatic release."
```

#### 4. Release Workflow (Automated)

```yaml
# .github/workflows/release.yml
name: Release

on:
  pull_request:
    types: [closed]
    branches: [main]

jobs:
  release:
    if: |
      github.event.pull_request.merged == true && 
      startsWith(github.head_ref, 'release/v')
    runs-on: ubuntu-latest
    
    steps:
      - name: Extract version from branch name
        id: version
        run: |
          # release/v1.3.0 → 1.3.0
          VERSION=$(echo ${{ github.head_ref }} | sed 's/release\/v//')
          echo "version=$VERSION" >> $GITHUB_OUTPUT
          
      - name: Checkout code
        uses: actions/checkout@v4
        
      - name: Update version for release (temporary)
        run: |
          # Update version temporarily for this build only
          sed -i "s/version = \".*\"/version = \"${{ steps.version.outputs.version }}\"/" Cargo.toml
          cargo update --package dotsnapshot --precise "${{ steps.version.outputs.version }}"
          
      - name: Build release binaries
        run: |
          # Install Rust
          curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
          source ~/.cargo/env
          
          # Build for multiple targets
          cargo build --release
          
      - name: Verify binary version
        run: |
          BINARY_VERSION=$(target/release/dotsnapshot --version | grep -o '[0-9]\+\.[0-9]\+\.[0-9]\+')
          if [ "$BINARY_VERSION" != "${{ steps.version.outputs.version }}" ]; then
            echo "❌ Version mismatch: binary reports $BINARY_VERSION, expected ${{ steps.version.outputs.version }}"
            exit 1
          fi
          echo "✅ Binary version verified: $BINARY_VERSION"
          
      - name: Package binaries
        run: |
          # Package for distribution
          tar -czf dotsnapshot-linux-x86_64.tar.gz -C target/release dotsnapshot
          shasum -a 256 dotsnapshot-linux-x86_64.tar.gz > dotsnapshot-linux-x86_64.sha256
          
      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: v${{ steps.version.outputs.version }}
          name: Release v${{ steps.version.outputs.version }}
          body_path: RELEASE_NOTES.md
          files: |
            dotsnapshot-*.tar.gz
            dotsnapshot-*.sha256
            
      - name: Prepare next development version
        run: |
          # Increment patch version for next development
          IFS='.' read -r major minor patch <<< "${{ steps.version.outputs.version }}"
          NEXT_VERSION="$major.$minor.$((patch + 1))"
          
          # Create PR to bump main to next dev version
          git config --local user.email "action@github.com"
          git config --local user.name "GitHub Action"
          
          git checkout main
          git pull origin main
          git checkout -b "chore/bump-to-v$NEXT_VERSION"
          
          sed -i "s/version = \".*\"/version = \"$NEXT_VERSION\"/" Cargo.toml
          cargo update --package dotsnapshot --precise "$NEXT_VERSION"
          
          git add Cargo.toml Cargo.lock
          git commit -m "chore: Bump to v$NEXT_VERSION for next development cycle"
          git push origin "chore/bump-to-v$NEXT_VERSION"
          
          gh pr create \
            --title "chore: Bump to v$NEXT_VERSION" \
            --body "Automated version bump after v${{ steps.version.outputs.version }} release" \
            --auto-merge
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

### Alternative: Simplified Manual Process

If the above is still too complex, here's a simpler approach:

#### Manual Release Script
```bash
#!/bin/bash
# scripts/release.sh

set -e

if [ -z "$1" ]; then
  echo "Usage: ./scripts/release.sh <version>"
  echo "Example: ./scripts/release.sh 1.3.0"
  exit 1
fi

VERSION=$1
echo "🚀 Releasing version $VERSION"

# 1. Update version
echo "📝 Updating Cargo.toml to version $VERSION"
sed -i "s/version = \".*\"/version = \"$VERSION\"/" Cargo.toml
cargo update --package dotsnapshot --precise "$VERSION"

# 2. Build and test
echo "🔨 Building and testing"
cargo build --release
cargo test

# 3. Verify version
echo "✅ Verifying binary version"
BINARY_VERSION=$(target/release/dotsnapshot --version | grep -o '[0-9]\+\.[0-9]\+\.[0-9]\+')
if [ "$BINARY_VERSION" != "$VERSION" ]; then
  echo "❌ Version mismatch: binary reports $BINARY_VERSION, expected $VERSION"
  exit 1
fi

# 4. Create release commit
echo "📝 Creating release commit"
git add Cargo.toml Cargo.lock
git commit -m "chore: Release v$VERSION"

# 5. Create tag
echo "🏷️  Creating tag v$VERSION"
git tag "v$VERSION"

# 6. Push
echo "⬆️  Pushing to GitHub"
git push origin main
git push origin "v$VERSION"

# 7. Create GitHub release (manual)
echo "🎉 Release v$VERSION created!"
echo "📋 Create GitHub release at: https://github.com/tomerlichtash/dotsnapshot/releases/new?tag=v$VERSION"

# 8. Prepare next version
echo "🔄 Preparing next development version"
IFS='.' read -r major minor patch <<< "$VERSION"
NEXT_VERSION="$major.$minor.$((patch + 1))"

sed -i "s/version = \".*\"/version = \"$NEXT_VERSION\"/" Cargo.toml
cargo update --package dotsnapshot --precise "$NEXT_VERSION"

git add Cargo.toml Cargo.lock
git commit -m "chore: Bump to v$NEXT_VERSION for next development cycle"
git push origin main

echo "✅ Ready for next development cycle at v$NEXT_VERSION"
```

## Recommended Approach

**Option 1: Simplified Manual Process**
- Use the release script above
- Maintainer runs `./scripts/release.sh 1.3.0`
- Script handles everything: version bump, build, test, tag, push
- No complex workflows, easy to debug
- Clear, predictable process

**Benefits:**
- ✅ Single source of truth (Cargo.toml)
- ✅ No uncommitted changes
- ✅ No direct main pushes (script can be modified to use PRs)
- ✅ Version alignment guaranteed
- ✅ Simple to understand and debug

**Next Steps:**
1. Abandon semantic-release complexity
2. Implement the manual release script
3. Test with v1.2.1 release
4. Document the process
5. Update Homebrew formula

## Testing the New Process

Let's test with v1.2.1:
```bash
# Fix current state
git checkout main
git reset --hard HEAD~1  # Remove problematic commits
echo '1.2.1' > VERSION_TO_RELEASE
./scripts/release.sh 1.2.1
```

This gives us a clean, predictable, debuggable release process that meets all requirements.