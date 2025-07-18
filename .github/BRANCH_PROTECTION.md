# Branch Protection Setup

This document describes how to set up branch protection rules for the main branch to ensure semantic release works correctly.

## Required Branch Protection Rules

To set up branch protection for the `main` branch, go to:
**Settings > Branches > Add rule**

### Required Settings

1. **Branch name pattern**: `main`

2. **Protect matching branches**:
   - ✅ Require a pull request before merging
   - ✅ Require approvals: 1
   - ✅ Dismiss stale PR approvals when new commits are pushed
   - ✅ Require review from code owners (if CODEOWNERS file exists)

3. **Require status checks to pass before merging**:
   - ✅ Require branches to be up to date before merging
   - ✅ Status checks that are required:
     - `CI / Test (ubuntu-latest, stable)`
     - `CI / Test (windows-latest, stable)`  
     - `CI / Test (macos-latest, stable)`
     - `CI / Test (ubuntu-latest, beta)`
     - `CI / Minimum Supported Rust Version`
     - `CI / Security audit`
     - `CI / Code coverage`
     - `Semantic Commits / Validate commit messages`

4. **Restrict pushes that create files that match a pattern**:
   - ✅ Restrict pushes
   - ✅ Patterns: `*` (to prevent direct pushes to main)

5. **Rules applied to everyone including administrators**:
   - ✅ Include administrators
   - ✅ Allow force pushes: **NO**
   - ✅ Allow deletions: **NO**

## Semantic Release Permissions

For semantic release to work correctly, you need to:

1. **Add the following repository secrets**:
   - `GITHUB_TOKEN`: Already available by default
   - `CRATES_IO_TOKEN`: Required for publishing to crates.io

2. **Grant semantic release bypass permissions**:
   - The semantic release process needs to push commits and tags to the protected main branch
   - This is handled automatically by using the `GITHUB_TOKEN` with proper permissions

## Manual Setup Commands

You can also set up branch protection via GitHub CLI:

```bash
# Enable branch protection for main
gh api repos/:owner/:repo/branches/main/protection \
  --method PUT \
  --raw-field required_status_checks='{"strict":true,"contexts":["CI / Test (ubuntu-latest, stable)","CI / Test (windows-latest, stable)","CI / Test (macos-latest, stable)","CI / Test (ubuntu-latest, beta)","CI / Minimum Supported Rust Version","CI / Security audit","CI / Code coverage","Semantic Commits / Validate commit messages"]}' \
  --raw-field enforce_admins='{"enabled":true}' \
  --raw-field required_pull_request_reviews='{"required_approving_review_count":1,"dismiss_stale_reviews":true}' \
  --raw-field restrictions='null'
```

## Important Notes

- **Never push directly to main**: All changes must go through pull requests
- **Semantic commits required**: All commit messages must follow conventional commit format
- **CI must pass**: All status checks must be green before merging
- **Releases are automatic**: Once commits are merged to main, semantic release will automatically create releases based on commit messages