#!/bin/bash

# =============================================================================
# Cursor Extensions Snapshot Script
# =============================================================================
# This script creates a snapshot of currently installed Cursor extensions
# with their versions.

# Source common utilities
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$(dirname "$SCRIPT_DIR")/lib/common.sh"

# =============================================================================
# Configuration
# =============================================================================

# File paths
CURSOR_EXTENSIONS_FILE="$LATEST_DIR/cursor_extensions"

# =============================================================================
# Main Functions
# =============================================================================

check_cursor_dependencies() {
    log "INFO" "Checking Cursor dependencies..."
    
    if ! check_dependency "Cursor" "cursor"; then
        log "ERROR" "Cursor is not installed or not in PATH"
        exit 1
    fi
    
    log "SUCCESS" "All Cursor dependencies are available"
}

snapshot_cursor_extensions() {
    log "STEP" "Starting Cursor extensions snapshot..."
    
    # Backup existing file (only if backups are enabled)
    backup_file "$CURSOR_EXTENSIONS_FILE" "cursor_extensions"
    
    # Remove existing file
    remove_file_if_exists "$CURSOR_EXTENSIONS_FILE" "cursor_extensions"
    
    # Create new snapshot
    log "INFO" "Creating new Cursor extensions snapshot..."
    log "INFO" "Running: cursor --list-extensions --show-versions > $CURSOR_EXTENSIONS_FILE"
    
    if cursor --list-extensions --show-versions > "$CURSOR_EXTENSIONS_FILE" 2>/dev/null; then
        log "SUCCESS" "Cursor extensions snapshot created successfully"
        
        # Count extensions
        local extension_count=$(wc -l < "$CURSOR_EXTENSIONS_FILE" 2>/dev/null || echo "0")
        log "INFO" "Snapshot contains $extension_count Cursor extensions"
    else
        log "ERROR" "Failed to create Cursor extensions snapshot"
        return 1
    fi
}

validate_cursor_snapshot() {
    log "INFO" "Validating Cursor extensions snapshot..."
    
    if validate_file "$CURSOR_EXTENSIONS_FILE" "cursor_extensions"; then
        log "SUCCESS" "Cursor extensions snapshot validated"
        return 0
    else
        log "ERROR" "Cursor extensions snapshot validation failed"
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
    init_snapshot "cursor-extensions-snapshot.sh" "cursor-extensions-snapshot.log" "$enable_backup" "$shared_timestamp"
    
    # Check dependencies
    check_cursor_dependencies
    
    # Create snapshot
    if snapshot_cursor_extensions; then
        log "SUCCESS" "Cursor extensions snapshot completed"
        
        # Validate snapshot
        if validate_cursor_snapshot; then
            log "SUCCESS" "Cursor extensions snapshot process finished successfully"
            log "INFO" "File created: $CURSOR_EXTENSIONS_FILE"
        else
            log "ERROR" "Cursor extensions snapshot validation failed"
            exit 1
        fi
    else
        log "ERROR" "Cursor extensions snapshot failed"
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