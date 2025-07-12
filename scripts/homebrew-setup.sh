#!/bin/bash

# =============================================================================
# Homebrew Formula Setup Script
# =============================================================================
# This script helps prepare DotSnapshot for Homebrew submission.
# 
# Usage:
#   ./scripts/homebrew-setup.sh [OPTIONS]
# 
# Options:
#   --version VERSION    Set version for release (default: from VERSION file)
#   --github USERNAME    Set GitHub username for repository
#   --create-release     Create GitHub release and calculate SHA256
#   --test-formula       Test the formula locally
#   --help               Show this help message

set -euo pipefail

# Script configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
FORMULA_DIR="$PROJECT_ROOT/Formula"
FORMULA_FILE="$FORMULA_DIR/dotsnapshot.rb"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
VERSION=""
GITHUB_USERNAME=""
CREATE_RELEASE=false
TEST_FORMULA=false

# =============================================================================
# Utility Functions
# =============================================================================

# =============================================================================
# log
# =============================================================================
# Simple logging function that outputs colored messages to the console.
# Used for consistent logging throughout the Homebrew setup script.
#
# Parameters:
#   $1 - level: Log level (INFO, SUCCESS, WARNING, ERROR, or custom)
#   $2 - message: The message to log
#
# Returns: None (outputs to console)
# =============================================================================
log() {
    local level="$1"
    local message="$2"
    
    case "$level" in
        "INFO")
            echo -e "${BLUE}[INFO]${NC} $message"
            ;;
        "SUCCESS")
            echo -e "${GREEN}[SUCCESS]${NC} $message"
            ;;
        "WARNING")
            echo -e "${YELLOW}[WARNING]${NC} $message"
            ;;
        "ERROR")
            echo -e "${RED}[ERROR]${NC} $message"
            ;;
        *)
            echo "[$level] $message"
            ;;
    esac
}

# =============================================================================
# show_help
# =============================================================================
# Displays comprehensive help information for the Homebrew setup script,
# including usage instructions, available options, examples, and submission
# steps. Used when --help is specified or when invalid options are provided.
#
# Parameters: None
# Returns: None (outputs help text to console)
# Side Effects: Prints formatted help information
# =============================================================================
show_help() {
    echo "Homebrew Formula Setup Script"
    echo "============================="
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --version VERSION    Set version for release (default: from VERSION file)"
    echo "  --github USERNAME    Set GitHub username for repository"
    echo "  --create-release     Create GitHub release and calculate SHA256"
    echo "  --test-formula       Test the formula locally"
    echo "  --help               Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0 --version 1.0.0 --github yourusername"
    echo "  $0 --create-release"
    echo "  $0 --test-formula"
    echo ""
    echo "Steps to submit to Homebrew:"
    echo "1. Update version and GitHub username"
    echo "2. Create GitHub release"
    echo "3. Calculate SHA256 hash"
    echo "4. Test formula locally"
    echo "5. Submit to Homebrew/homebrew-core"
}

# =============================================================================
# parse_arguments
# =============================================================================
# Parses command line arguments and sets global variables accordingly.
# Handles all supported options and provides appropriate error messages
# for invalid or missing arguments.
#
# Parameters: All command line arguments ($@)
# Returns: None
# Side Effects: Sets global variables (VERSION, GITHUB_USERNAME, CREATE_RELEASE, TEST_FORMULA)
# Exit Code: 1 on error, 0 on success
# =============================================================================
parse_arguments() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --version)
                if [[ -z "${2:-}" ]]; then
                    log "ERROR" "Version required"
                    exit 1
                fi
                VERSION="$2"
                shift 2
                ;;
            --github)
                if [[ -z "${2:-}" ]]; then
                    log "ERROR" "GitHub username required"
                    exit 1
                fi
                GITHUB_USERNAME="$2"
                shift 2
                ;;
            --create-release)
                CREATE_RELEASE=true
                shift
                ;;
            --test-formula)
                TEST_FORMULA=true
                shift
                ;;
            --help|-h)
                show_help
                exit 0
                ;;
            *)
                log "ERROR" "Unknown option: $1"
                echo ""
                show_help
                exit 1
                ;;
        esac
    done
}

# =============================================================================
# get_version
# =============================================================================
# Retrieves the version to use for the Homebrew formula. Prioritizes
# command line argument, then falls back to reading from VERSION file.
# Exits with error if no version can be determined.
#
# Parameters: None
# Returns: Version string
# Exit Code: 1 if no version found, 0 on success
# Example: get_version returns "1.0.0"
# =============================================================================
get_version() {
    if [[ -n "$VERSION" ]]; then
        echo "$VERSION"
    elif [[ -f "$PROJECT_ROOT/VERSION" ]]; then
        tr -d ' \t\n\r' < "$PROJECT_ROOT/VERSION"
    else
        log "ERROR" "No version specified and VERSION file not found"
        exit 1
    fi
}

# =============================================================================
# get_github_username
# =============================================================================
# Retrieves the GitHub username to use for the Homebrew formula. Prioritizes
# command line argument, then attempts to extract from git remote URL.
# Exits with error if no username can be determined.
#
# Parameters: None
# Returns: GitHub username string
# Exit Code: 1 if no username found, 0 on success
# Example: get_github_username returns "tomerlichtash"
# =============================================================================
get_github_username() {
    if [[ -n "$GITHUB_USERNAME" ]]; then
        echo "$GITHUB_USERNAME"
    else
        # Try to get from git remote
        local remote_url=$(git remote get-url origin 2>/dev/null || echo "")
        if [[ "$remote_url" =~ github\.com[:/]([^/]+)/dotsnapshot ]]; then
            echo "${BASH_REMATCH[1]}"
        else
            log "ERROR" "GitHub username not specified and cannot be determined from git remote"
            log "INFO" "Use --github USERNAME to specify your GitHub username"
            exit 1
        fi
    fi
}

# =============================================================================
# update_formula
# =============================================================================
# Updates the Homebrew formula file with the current version and GitHub username.
# Uses sed to replace placeholders in the formula template with actual values.
# Creates the Formula directory if it doesn't exist.
#
# Parameters: None
# Returns: None
# Side Effects: 
#   - Creates Formula directory if needed
#   - Updates formula file with version and username
#   - Creates backup of original formula file
# =============================================================================
update_formula() {
    local version=$(get_version)
    local username=$(get_github_username)
    
    log "INFO" "Updating formula with version: $version, username: $username"
    
    # Create Formula directory if it doesn't exist
    mkdir -p "$FORMULA_DIR"
    
    # Update the formula file
    sed -i.bak \
        -e "s|yourusername|$username|g" \
        -e "s|v1.0.0|v$version|g" \
        -e "s|1.0.0.tar.gz|$version.tar.gz|g" \
        "$FORMULA_FILE"
    
    rm -f "$FORMULA_FILE.bak"
    
    log "SUCCESS" "Formula updated: $FORMULA_FILE"
}

# =============================================================================
# create_github_release
# =============================================================================
# Creates a GitHub release by creating and pushing a git tag. Checks for
# clean working directory and existing tags before proceeding. Waits for
# GitHub to create the release before calculating SHA256.
#
# Parameters: None
# Returns: None
# Side Effects: 
#   - Creates git tag
#   - Pushes tag to remote repository
#   - Calls calculate_sha256 after release creation
# Exit Code: 1 on error, 0 on success
# =============================================================================
create_github_release() {
    local version=$(get_version)
    local username=$(get_github_username)
    
    log "INFO" "Creating GitHub release for version: $version"
    
    # Check if git is clean
    if [[ -n "$(git status --porcelain)" ]]; then
        log "ERROR" "Git working directory is not clean"
        log "INFO" "Please commit or stash your changes before creating a release"
        exit 1
    fi
    
    # Check if tag already exists
    local tag_name="v$version"
    if git tag -l "$tag_name" | grep -q "$tag_name"; then
        log "WARNING" "Git tag $tag_name already exists"
        read -p "Do you want to overwrite it? (y/N): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            log "INFO" "Release creation cancelled"
            return 0
        fi
        git tag -d "$tag_name" 2>/dev/null || true
    fi
    
    # Create tag
    git tag -a "$tag_name" -m "Release version $version"
    log "SUCCESS" "Git tag created: $tag_name"
    
    # Push tag
    if git push origin "$tag_name"; then
        log "SUCCESS" "Git tag pushed to remote"
    else
        log "ERROR" "Failed to push git tag"
        exit 1
    fi
    
    # Wait for GitHub to create the release
    log "INFO" "Waiting for GitHub to create release..."
    sleep 10
    
    # Calculate SHA256
    calculate_sha256
}

# =============================================================================
# calculate_sha256
# =============================================================================
# Downloads the release tarball from GitHub and calculates its SHA256 hash.
# Updates the formula file with the calculated hash for Homebrew verification.
# This function is called after creating a GitHub release.
#
# Parameters: None
# Returns: None
# Side Effects: 
#   - Downloads tarball from GitHub
#   - Updates formula file with SHA256 hash
#   - Displays hash and URL information
# Exit Code: 1 on error, 0 on success
# =============================================================================
calculate_sha256() {
    local version=$(get_version)
    local username=$(get_github_username)
    local url="https://github.com/$username/dotsnapshot/archive/refs/tags/v$version.tar.gz"
    
    log "INFO" "Calculating SHA256 for: $url"
    
    # Download and calculate SHA256
    local sha256=$(curl -sL "$url" | shasum -a 256 | cut -d' ' -f1)
    
    if [[ -n "$sha256" ]]; then
        log "SUCCESS" "SHA256 calculated: $sha256"
        
        # Update formula with SHA256
        sed -i.bak "s|YOUR_SHA256_HERE|$sha256|g" "$FORMULA_FILE"
        rm -f "$FORMULA_FILE.bak"
        
        log "SUCCESS" "Formula updated with SHA256"
        echo ""
        echo "Updated formula file: $FORMULA_FILE"
        echo "SHA256: $sha256"
        echo "URL: $url"
    else
        log "ERROR" "Failed to calculate SHA256"
        exit 1
    fi
}

# =============================================================================
# test_formula
# =============================================================================
# Tests the Homebrew formula locally to ensure it's valid and can be installed.
# Performs syntax validation using Ruby and a dry-run installation test using
# Homebrew. Exits with error if any test fails.
#
# Parameters: None
# Returns: None
# Side Effects: Runs Homebrew commands for testing
# Exit Code: 1 on test failure, 0 on success
# =============================================================================
test_formula() {
    log "INFO" "Testing formula locally..."
    
    if [[ ! -f "$FORMULA_FILE" ]]; then
        log "ERROR" "Formula file not found: $FORMULA_FILE"
        exit 1
    fi
    
    # Test formula syntax using ruby syntax check
    log "INFO" "Testing formula syntax..."
    if ruby -c "$FORMULA_FILE" 2>/dev/null; then
        log "SUCCESS" "Formula syntax is valid"
    else
        log "ERROR" "Formula syntax is invalid"
        exit 1
    fi
    
    # Test formula installation (dry run)
    log "INFO" "Testing formula installation (dry run)..."
    if brew install --dry-run "$FORMULA_FILE"; then
        log "SUCCESS" "Formula installation test passed"
    else
        log "ERROR" "Formula installation test failed"
        exit 1
    fi
}

# =============================================================================
# show_submission_instructions
# =============================================================================
# Displays step-by-step instructions for submitting the formula to Homebrew.
# Provides the exact commands and URLs needed to complete the submission
# process to homebrew-core.
#
# Parameters: None
# Returns: None (outputs instructions to console)
# Side Effects: Prints formatted submission instructions
# =============================================================================
show_submission_instructions() {
    echo ""
    echo "Homebrew Submission Instructions"
    echo "================================"
    echo ""
    echo "1. Fork the Homebrew/homebrew-core repository:"
    echo "   https://github.com/Homebrew/homebrew-core"
    echo ""
    echo "2. Copy your formula to the Formula directory:"
    echo "   cp $FORMULA_FILE /path/to/homebrew-core/Formula/"
    echo ""
    echo "3. Commit and push your changes:"
    echo "   cd /path/to/homebrew-core"
    echo "   git add Formula/dotsnapshot.rb"
    echo "   git commit -m 'dotsnapshot 1.0.0'"
    echo "   git push origin your-branch"
    echo ""
    echo "4. Create a pull request:"
    echo "   https://github.com/Homebrew/homebrew-core/compare"
    echo ""
    echo "5. Wait for review and merge"
    echo ""
    echo "After acceptance, users can install with:"
    echo "   brew install dotsnapshot"
}

# =============================================================================
# Main Execution
# =============================================================================

# =============================================================================
# main
# =============================================================================
# Main entry point for the Homebrew setup script. Orchestrates the entire
# workflow including argument parsing, formula updates, release creation,
# testing, and providing submission instructions.
#
# Parameters: All command line arguments ($@)
# Returns: None
# Side Effects: 
#   - Updates formula files
#   - Creates GitHub releases
#   - Runs formula tests
#   - Displays instructions
# =============================================================================
main() {
    log "INFO" "Starting Homebrew formula setup..."
    
    # Parse command line arguments
    parse_arguments "$@"
    
    # Update formula with version and username
    if [[ -n "$VERSION" ]] || [[ -n "$GITHUB_USERNAME" ]]; then
        update_formula
    fi
    
    # Create GitHub release if requested
    if [[ "$CREATE_RELEASE" == "true" ]]; then
        create_github_release
    fi
    
    # Test formula if requested
    if [[ "$TEST_FORMULA" == "true" ]]; then
        test_formula
    fi
    
    # Show submission instructions
    show_submission_instructions
}

# =============================================================================
# Script Execution
# =============================================================================

# Only run main if script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi