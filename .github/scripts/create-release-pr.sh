#!/bin/bash
set -e

RELEASE_VERSION="$1"
RELEASE_NOTES="$2"

# Create PR using GitHub API
curl -X POST \
  -H "Authorization: token $GITHUB_TOKEN" \
  -H "Accept: application/vnd.github.v3+json" \
  https://api.github.com/repos/tomerlichtash/dotsnapshot/pulls \
  -d "{
    \"title\": \"chore(release): ${RELEASE_VERSION} [RELEASE]\",
    \"body\": \"Automated release PR for version ${RELEASE_VERSION}\\n\\n${RELEASE_NOTES}\",
    \"head\": \"release/v${RELEASE_VERSION}\",
    \"base\": \"main\"
  }" > pr_response.json

# Extract PR number and merge it
PR_NUMBER=$(cat pr_response.json | grep '"number":' | head -1 | sed 's/.*"number": *\([0-9]*\).*/\1/')

curl -X PUT \
  -H "Authorization: token $GITHUB_TOKEN" \
  -H "Accept: application/vnd.github.v3+json" \
  https://api.github.com/repos/tomerlichtash/dotsnapshot/pulls/$PR_NUMBER/merge \
  -d '{"merge_method": "squash"}'