#!/bin/bash
set -e

# Configure git for GitHub Actions bot
git config --global user.name 'github-actions[bot]'
git config --global user.email '41898282+github-actions[bot]@users.noreply.github.com'

# Create and push release branch
RELEASE_VERSION="$1"
git checkout -b "release/v${RELEASE_VERSION}"
git add CHANGELOG.md Cargo.toml Cargo.lock
git commit -m "chore(release): ${RELEASE_VERSION} [skip ci]"
git push origin "release/v${RELEASE_VERSION}"