#!/bin/bash
# Release script for dotsnapshot
# Usage: ./scripts/release.sh <version>

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

log_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

log_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

log_error() {
    echo -e "${RED}❌ $1${NC}"
}

if [ -z "$1" ]; then
  log_error "Usage: ./scripts/release.sh <version>"
  log_info "Example: ./scripts/release.sh 1.3.0"
  exit 1
fi

VERSION=$1
log_info "Starting release process for version $VERSION"

# Verify we're on main branch
CURRENT_BRANCH=$(git branch --show-current)
if [ "$CURRENT_BRANCH" != "main" ]; then
    log_error "Must be on main branch. Currently on: $CURRENT_BRANCH"
    exit 1
fi

# Verify clean working directory
if [ -n "$(git status --porcelain)" ]; then
    log_error "Working directory must be clean. Please commit or stash changes."
    git status
    exit 1
fi

# Pull latest changes
log_info "Pulling latest changes from origin/main"
git pull origin main

# 1. Update version in Cargo.toml
log_info "Updating Cargo.toml to version $VERSION"
sed -i '' "s/version = \".*\"/version = \"$VERSION\"/" Cargo.toml

# Update Cargo.lock
log_info "Updating Cargo.lock"
cargo update --package dotsnapshot --precise "$VERSION"

# 2. Build and test
log_info "Building release binary"
cargo build --release

log_info "Running tests"
cargo test

# 3. Verify version matches
log_info "Verifying binary version"
BINARY_VERSION=$(target/release/dotsnapshot --version | grep -o '[0-9]\+\.[0-9]\+\.[0-9]\+')
if [ "$BINARY_VERSION" != "$VERSION" ]; then
  log_error "Version mismatch: binary reports $BINARY_VERSION, expected $VERSION"
  exit 1
fi
log_success "Binary version verified: $BINARY_VERSION"

# 4. Generate changelog for this release
log_info "Generating release notes"
LAST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "")
if [ -n "$LAST_TAG" ]; then
    log_info "Generating changelog since $LAST_TAG"
    echo "# Release v$VERSION" > RELEASE_NOTES_TEMP.md
    echo "" >> RELEASE_NOTES_TEMP.md
    git log --oneline "${LAST_TAG}..HEAD" --pretty=format:"- %s" >> RELEASE_NOTES_TEMP.md
    echo "" >> RELEASE_NOTES_TEMP.md
else
    log_warning "No previous tags found, creating initial release notes"
    echo "# Release v$VERSION" > RELEASE_NOTES_TEMP.md
    echo "" >> RELEASE_NOTES_TEMP.md
    echo "Initial release" >> RELEASE_NOTES_TEMP.md
fi

# 5. Create release branch and PR (no direct main pushes)
RELEASE_BRANCH="release/v$VERSION"
log_info "Creating release branch: $RELEASE_BRANCH"
git checkout -b "$RELEASE_BRANCH"

# Commit version changes
git add Cargo.toml Cargo.lock
git commit -m "chore: Release v$VERSION

Updates version to $VERSION for release"

# Add release notes if they exist
if [ -f "RELEASE_NOTES_TEMP.md" ]; then
    mv RELEASE_NOTES_TEMP.md "RELEASE_NOTES_v$VERSION.md"
    git add "RELEASE_NOTES_v$VERSION.md"
    git commit -m "docs: Add release notes for v$VERSION"
fi

# Push release branch
log_info "Pushing release branch to origin"
git push origin "$RELEASE_BRANCH"

# Create PR
log_info "Creating release PR"
PR_URL=$(gh pr create \
    --title "Release v$VERSION" \
    --body "🚀 Release version $VERSION

## Changes
$(cat RELEASE_NOTES_v$VERSION.md | tail -n +3)

## Checklist
- [x] Version updated in Cargo.toml
- [x] Cargo.lock updated
- [x] Binary version verified
- [x] Tests passing
- [x] Release notes generated

**⚠️ This PR will trigger the release workflow when merged.**" \
    --label "release")

log_success "Release PR created: $PR_URL"

# Switch back to main
git checkout main

log_info "Release process initiated!"
log_info "Next steps:"
log_info "1. Review the PR: $PR_URL"
log_info "2. Merge the PR to trigger release workflow"
log_info "3. GitHub will create the release and build binaries"
log_info "4. Update Homebrew formula to point to v$VERSION"

log_success "Release v$VERSION ready for review!"