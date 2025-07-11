#!/bin/bash

# =============================================================================
# VS Code Extensions Snapshot Script
# =============================================================================
# This script creates a snapshot of currently installed VS Code extensions
# with their versions.

# Source common utilities
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$(dirname "$SCRIPT_DIR")/lib/common.sh"

# =============================================================================
# Configuration
# =============================================================================

# File paths
VSCODE_EXTENSIONS_FILE="$LATEST_DIR/vscode_extensions"

# =============================================================================
# Main Functions
# =============================================================================

check_vscode_dependencies() {
    log "INFO" "Checking VS Code dependencies..."
    
    if ! check_dependency "VS Code" "code"; then
        log "ERROR" "VS Code is not installed or not in PATH"
        exit 1
    fi
    
    log "SUCCESS" "All VS Code dependencies are available"
}

snapshot_vscode_extensions() {
    log "STEP" "Starting VS Code extensions snapshot..."
    
    # Backup existing file (only if backups are enabled)
    backup_file "$VSCODE_EXTENSIONS_FILE" "vscode_extensions"
    
    # Remove existing file
    remove_file_if_exists "$VSCODE_EXTENSIONS_FILE" "vscode_extensions"
    
    # Create new snapshot
    log "INFO" "Creating new VS Code extensions snapshot..."
    log "INFO" "Running: code --list-extensions --show-versions > $VSCODE_EXTENSIONS_FILE"
    
    if code --list-extensions --show-versions > "$VSCODE_EXTENSIONS_FILE" 2>/dev/null; then
        log "SUCCESS" "VS Code extensions snapshot created successfully"
        
        # Count extensions
        local extension_count=$(wc -l < "$VSCODE_EXTENSIONS_FILE" 2>/dev/null || echo "0")
        log "INFO" "Snapshot contains $extension_count VS Code extensions"
    else
        log "ERROR" "Failed to create VS Code extensions snapshot"
        return 1
    fi
}

validate_vscode_snapshot() {
    log "INFO" "Validating VS Code extensions snapshot..."
    
    if validate_file "$VSCODE_EXTENSIONS_FILE" "vscode_extensions"; then
        log "SUCCESS" "VS Code extensions snapshot validated"
        return 0
    else
        log "ERROR" "VS Code extensions snapshot validation failed"
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
    init_snapshot "vscode-extensions-snapshot.sh" "vscode-extensions-snapshot.log" "$enable_backup" "$shared_timestamp"
    
    # Check dependencies
    check_vscode_dependencies
    
    # Create snapshot
    if snapshot_vscode_extensions; then
        log "SUCCESS" "VS Code extensions snapshot completed"
        
        # Validate snapshot
        if validate_vscode_snapshot; then
            log "SUCCESS" "VS Code extensions snapshot process finished successfully"
            log "INFO" "File created: $VSCODE_EXTENSIONS_FILE"
        else
            log "ERROR" "VS Code extensions snapshot validation failed"
            exit 1
        fi
    else
        log "ERROR" "VS Code extensions snapshot failed"
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