#!/bin/bash

# =============================================================================
# VS Code Settings Snapshot Script
# =============================================================================
# This script creates a snapshot of VS Code's settings.json file.

# Source common utilities
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$(dirname "$SCRIPT_DIR")/lib/common.sh"

# =============================================================================
# Configuration
# =============================================================================

# File paths
VSCODE_DOTFILES_SETTINGS="$PROJECT_ROOT/settings/vscode/settings.json"
VSCODE_USER_SETTINGS="$HOME/Library/Application Support/Code/User/settings.json"
VSCODE_SETTINGS_FILE="$LATEST_DIR/vscode_settings.json"

# Determine which settings file to use (prioritize dotfiles version)
if [[ -f "$VSCODE_DOTFILES_SETTINGS" ]]; then
    VSCODE_SETTINGS_SOURCE="$VSCODE_DOTFILES_SETTINGS"
    SETTINGS_TYPE="dotfiles"
elif [[ -f "$VSCODE_USER_SETTINGS" ]]; then
    VSCODE_SETTINGS_SOURCE="$VSCODE_USER_SETTINGS"
    SETTINGS_TYPE="user"
else
    VSCODE_SETTINGS_SOURCE="$VSCODE_USER_SETTINGS"
    SETTINGS_TYPE="default"
fi

# =============================================================================
# Main Functions
# =============================================================================

check_vscode_settings_dependencies() {
    log "INFO" "Checking VS Code settings dependencies..."
    
    # Check if VS Code settings file exists
    if [[ ! -f "$VSCODE_SETTINGS_SOURCE" ]]; then
        log "ERROR" "VS Code settings file not found: $VSCODE_SETTINGS_SOURCE"
        log "INFO" "This might mean VS Code is not installed or has not been configured yet"
        exit 1
    fi
    
    log "SUCCESS" "VS Code settings file found ($SETTINGS_TYPE settings)"
    log "INFO" "Using settings from: $VSCODE_SETTINGS_SOURCE"
}

snapshot_vscode_settings() {
    log "STEP" "Starting VS Code settings snapshot..."
    
    # Backup existing file (only if backups are enabled)
    backup_file "$VSCODE_SETTINGS_FILE" "vscode_settings.json"
    
    # Remove existing file
    remove_file_if_exists "$VSCODE_SETTINGS_FILE" "vscode_settings.json"
    
    # Create new snapshot
    log "INFO" "Creating new VS Code settings snapshot..."
    log "INFO" "Source type: $SETTINGS_TYPE settings"
    log "INFO" "Copying: $VSCODE_SETTINGS_SOURCE -> $VSCODE_SETTINGS_FILE"
    
    if cp "$VSCODE_SETTINGS_SOURCE" "$VSCODE_SETTINGS_FILE"; then
        log "SUCCESS" "VS Code settings snapshot created successfully"
        
        # Get file size
        local file_size=$(wc -c < "$VSCODE_SETTINGS_FILE" 2>/dev/null || echo "0")
        log "INFO" "Settings file size: $file_size bytes"
        
        # Validate JSON format
        if command -v jq &> /dev/null; then
            if jq empty "$VSCODE_SETTINGS_FILE" 2>/dev/null; then
                log "SUCCESS" "Settings file is valid JSON"
            else
                log "WARNING" "Settings file may not be valid JSON (jq validation failed)"
            fi
        else
            log "INFO" "jq not available - skipping JSON validation"
        fi
    else
        log "ERROR" "Failed to create VS Code settings snapshot"
        return 1
    fi
}

validate_vscode_settings_snapshot() {
    log "INFO" "Validating VS Code settings snapshot..."
    
    if validate_file "$VSCODE_SETTINGS_FILE" "vscode_settings.json"; then
        log "SUCCESS" "VS Code settings snapshot validated"
        return 0
    else
        log "ERROR" "VS Code settings snapshot validation failed"
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
    init_snapshot "vscode-settings-snapshot.sh" "vscode-settings-snapshot.log" "$enable_backup" "$shared_timestamp"
    
    # Check dependencies
    check_vscode_settings_dependencies
    
    # Create snapshot
    if snapshot_vscode_settings; then
        log "SUCCESS" "VS Code settings snapshot completed"
        
        # Validate snapshot
        if validate_vscode_settings_snapshot; then
            log "SUCCESS" "VS Code settings snapshot process finished successfully"
            log "INFO" "File created: $VSCODE_SETTINGS_FILE"
        else
            log "ERROR" "VS Code settings snapshot validation failed"
            exit 1
        fi
    else
        log "ERROR" "VS Code settings snapshot failed"
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