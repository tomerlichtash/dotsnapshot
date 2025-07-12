#!/bin/bash

# =============================================================================
# DotSnapshot Version Management Script
# =============================================================================
# This script helps manage versioning for the DotSnapshot project.
# 
# Usage:
#   ./scripts/version.sh                    # Show current version
#   ./scripts/version.sh bump major         # Bump major version
#   ./scripts/version.sh bump minor         # Bump minor version
#   ./scripts/version.sh bump patch         # Bump patch version
#   ./scripts/version.sh set 1.2.3          # Set specific version
#   ./scripts/version.sh tag                # Create git tag for current version

set -euo pipefail

# Script configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
VERSION_FILE="$PROJECT_ROOT/VERSION"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# =============================================================================
# Utility Functions
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

get_current_version() {
    if [[ -f "$VERSION_FILE" ]]; then
        tr -d ' \t\n\r' < "$VERSION_FILE"
    else
        log "ERROR" "VERSION file not found: $VERSION_FILE"
        exit 1
    fi
}

set_version() {
    local new_version="$1"
    
    # Validate version format (basic semver check)
    if [[ ! "$new_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.-]+)?(\+[a-zA-Z0-9.-]+)?$ ]]; then
        log "ERROR" "Invalid version format: $new_version"
        log "INFO" "Expected format: MAJOR.MINOR.PATCH[-PRERELEASE][+BUILD]"
        exit 1
    fi
    
    # Update VERSION file
    echo "$new_version" > "$VERSION_FILE"
    log "SUCCESS" "Version updated to: $new_version"
}

bump_version() {
    local bump_type="$1"
    local current_version=$(get_current_version)
    
    # Parse current version
    local major minor patch prerelease build
    if [[ "$current_version" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)(-[a-zA-Z0-9.-]+)?(\+[a-zA-Z0-9.-]+)?$ ]]; then
        major="${BASH_REMATCH[1]}"
        minor="${BASH_REMATCH[2]}"
        patch="${BASH_REMATCH[3]}"
        prerelease="${BASH_REMATCH[4]}"
        build="${BASH_REMATCH[5]}"
    else
        log "ERROR" "Invalid current version format: $current_version"
        exit 1
    fi
    
    # Calculate new version based on bump type
    local new_version
    case "$bump_type" in
        "major")
            new_version="$((major + 1)).0.0"
            ;;
        "minor")
            new_version="$major.$((minor + 1)).0"
            ;;
        "patch")
            new_version="$major.$minor.$((patch + 1))"
            ;;
        *)
            log "ERROR" "Invalid bump type: $bump_type"
            log "INFO" "Valid types: major, minor, patch"
            exit 1
            ;;
    esac
    
    # Preserve prerelease and build metadata if present
    if [[ -n "$prerelease" ]]; then
        new_version="${new_version}${prerelease}"
    fi
    if [[ -n "$build" ]]; then
        new_version="${new_version}${build}"
    fi
    
    set_version "$new_version"
}

create_git_tag() {
    local version=$(get_current_version)
    local tag_name="v$version"
    
    # Check if tag already exists
    if git tag -l "$tag_name" | grep -q "$tag_name"; then
        log "WARNING" "Git tag $tag_name already exists"
        read -p "Do you want to overwrite it? (y/N): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            log "INFO" "Tag creation cancelled"
            return 0
        fi
        git tag -d "$tag_name" 2>/dev/null || true
    fi
    
    # Create tag
    if git tag -a "$tag_name" -m "Release version $version"; then
        log "SUCCESS" "Git tag created: $tag_name"
        
        # Ask if user wants to push the tag
        read -p "Do you want to push the tag to remote? (y/N): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            if git push origin "$tag_name"; then
                log "SUCCESS" "Tag pushed to remote: $tag_name"
            else
                log "ERROR" "Failed to push tag to remote"
            fi
        fi
    else
        log "ERROR" "Failed to create git tag"
        exit 1
    fi
}

show_version_info() {
    local version=$(get_current_version)
    
    echo "DotSnapshot Version Information"
    echo "================================"
    echo "Current Version: $version"
    echo "Version File: $VERSION_FILE"
    echo ""
    
    # Parse version components
    if [[ "$version" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)(-[a-zA-Z0-9.-]+)?(\+[a-zA-Z0-9.-]+)?$ ]]; then
        local major="${BASH_REMATCH[1]}"
        local minor="${BASH_REMATCH[2]}"
        local patch="${BASH_REMATCH[3]}"
        local prerelease="${BASH_REMATCH[4]}"
        local build="${BASH_REMATCH[5]}"
        
        echo "Version Components:"
        echo "  Major: $major"
        echo "  Minor: $minor"
        echo "  Patch: $patch"
        if [[ -n "$prerelease" ]]; then
            echo "  Prerelease: $prerelease"
        fi
        if [[ -n "$build" ]]; then
            echo "  Build: $build"
        fi
        echo ""
    fi
    
    # Check git status
    if git rev-parse --git-dir > /dev/null 2>&1; then
        echo "Git Information:"
        echo "  Repository: $(git remote get-url origin 2>/dev/null || echo 'No remote')"
        echo "  Branch: $(git branch --show-current)"
        echo "  Commit: $(git rev-parse --short HEAD)"
        
        # Check if current version is tagged
        local tag_name="v$version"
        if git tag -l "$tag_name" | grep -q "$tag_name"; then
            echo "  Tagged: Yes ($tag_name)"
        else
            echo "  Tagged: No"
        fi
    else
        echo "Git Information: Not a git repository"
    fi
}

show_help() {
    echo "DotSnapshot Version Management"
    echo "=============================="
    echo ""
    echo "Usage: $0 [COMMAND] [ARGUMENTS]"
    echo ""
    echo "Commands:"
    echo "  show                    Show current version and information"
    echo "  bump <type>             Bump version (major|minor|patch)"
    echo "  set <version>           Set specific version"
    echo "  tag                     Create git tag for current version"
    echo "  help                    Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                      # Show current version"
    echo "  $0 bump minor           # Bump minor version (1.0.0 → 1.1.0)"
    echo "  $0 set 2.0.0            # Set version to 2.0.0"
    echo "  $0 tag                  # Create git tag for current version"
    echo ""
    echo "Version Format:"
    echo "  Follows Semantic Versioning 2.0.0"
    echo "  Format: MAJOR.MINOR.PATCH[-PRERELEASE][+BUILD]"
    echo "  Example: 1.2.3-alpha.1+build.123"
}

# =============================================================================
# Main Execution
# =============================================================================

main() {
    case "${1:-show}" in
        "show"|"")
            show_version_info
            ;;
        "bump")
            if [[ -z "${2:-}" ]]; then
                log "ERROR" "Bump type required"
                log "INFO" "Valid types: major, minor, patch"
                exit 1
            fi
            bump_version "$2"
            ;;
        "set")
            if [[ -z "${2:-}" ]]; then
                log "ERROR" "Version required"
                exit 1
            fi
            set_version "$2"
            ;;
        "tag")
            create_git_tag
            ;;
        "help"|"-h"|"--help")
            show_help
            ;;
        *)
            log "ERROR" "Unknown command: $1"
            echo ""
            show_help
            exit 1
            ;;
    esac
}

# =============================================================================
# Script Execution
# =============================================================================

# Only run main if script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi