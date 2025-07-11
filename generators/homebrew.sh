#!/bin/bash

# =============================================================================
# Homebrew Brewfile Snapshot Script
# =============================================================================
# This script creates a snapshot of currently installed Homebrew packages
# and saves them to a Brewfile.

# Source common utilities
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$(dirname "$SCRIPT_DIR")/lib/common.sh"

# =============================================================================
# Configuration
# =============================================================================

# File paths
BREWFILE_PATH="$LATEST_DIR/Brewfile"

# =============================================================================
# Main Functions
# =============================================================================

check_homebrew_dependencies() {
    log "INFO" "Checking Homebrew dependencies..."
    
    if ! check_dependency "Homebrew" "brew"; then
        log "ERROR" "Homebrew is not installed or not in PATH"
        exit 1
    fi
    
    log "SUCCESS" "All Homebrew dependencies are available"
}

snapshot_brewfile() {
    log "STEP" "Starting Homebrew Brewfile snapshot..."
    
    # Backup existing file (only if backups are enabled)
    backup_file "$BREWFILE_PATH" "Brewfile"
    
    # Remove existing file
    remove_file_if_exists "$BREWFILE_PATH" "Brewfile"
    
    # Create new snapshot
    log "INFO" "Creating new Brewfile snapshot..."
    log "INFO" "Running: brew bundle dump --force --file $BREWFILE_PATH"
    
    if brew bundle dump --force --file "$BREWFILE_PATH"; then
        log "SUCCESS" "Brewfile created successfully"
        
        # Count packages
        local package_count=$(grep -c '^[^#]' "$BREWFILE_PATH" 2>/dev/null || echo "0")
        log "INFO" "Brewfile contains $package_count package entries"
        
        # Show package types
        if [[ -s "$BREWFILE_PATH" ]]; then
            local brew_count=$(grep -c '^brew ' "$BREWFILE_PATH" 2>/dev/null || echo "0")
            local cask_count=$(grep -c '^cask ' "$BREWFILE_PATH" 2>/dev/null || echo "0")
            local tap_count=$(grep -c '^tap ' "$BREWFILE_PATH" 2>/dev/null || echo "0")
            
            log "INFO" "Package breakdown:"
            log "INFO" "  - Brew formulas: $brew_count"
            log "INFO" "  - Casks: $cask_count"
            log "INFO" "  - Taps: $tap_count"
        fi
    else
        log "ERROR" "Failed to create Brewfile snapshot"
        return 1
    fi
}

validate_brewfile_snapshot() {
    log "INFO" "Validating Brewfile snapshot..."
    
    if validate_file "$BREWFILE_PATH" "Brewfile"; then
        log "SUCCESS" "Brewfile snapshot validated"
        return 0
    else
        log "ERROR" "Brewfile snapshot validation failed"
        return 1
    fi
}

# =============================================================================
# Main Execution
# =============================================================================

main() {
    # Initialize snapshot (backup disabled by default for individual scripts)
    local enable_backup="${1:-false}"
    local shared_timestamp="${2:-}"
    init_snapshot "homebrew-snapshot.sh" "homebrew-snapshot.log" "$enable_backup" "$shared_timestamp"
    
    # Check dependencies
    check_homebrew_dependencies
    
    # Create snapshot
    if snapshot_brewfile; then
        log "SUCCESS" "Brewfile snapshot completed"
        
        # Validate snapshot
        if validate_brewfile_snapshot; then
            log "SUCCESS" "Brewfile snapshot process finished successfully"
            log "INFO" "File created: $BREWFILE_PATH"
        else
            log "ERROR" "Brewfile snapshot validation failed"
            exit 1
        fi
    else
        log "ERROR" "Brewfile snapshot failed"
        exit 1
    fi
}

# =============================================================================
# Script Execution
# =============================================================================

# Only run main if script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi