#!/bin/bash

# =============================================================================
# Cursor Settings Snapshot Script
# =============================================================================
# This script creates a snapshot of Cursor's settings.json file.

# Source common utilities
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$(dirname "$SCRIPT_DIR")/lib/common.sh"

# =============================================================================
# Configuration
# =============================================================================

# File paths
CURSOR_DOTFILES_SETTINGS="$DOTFILES_ROOT/settings/cursor/settings.json"
CURSOR_USER_SETTINGS="$HOME/Library/Application Support/Cursor/User/settings.json"
CURSOR_SETTINGS_FILE="$LATEST_DIR/cursor_settings.json"

# Determine which settings file to use (prioritize dotfiles version)
if [[ -f "$CURSOR_DOTFILES_SETTINGS" ]]; then
    CURSOR_SETTINGS_SOURCE="$CURSOR_DOTFILES_SETTINGS"
    SETTINGS_TYPE="dotfiles"
elif [[ -f "$CURSOR_USER_SETTINGS" ]]; then
    CURSOR_SETTINGS_SOURCE="$CURSOR_USER_SETTINGS"
    SETTINGS_TYPE="user"
else
    CURSOR_SETTINGS_SOURCE="$CURSOR_USER_SETTINGS"
    SETTINGS_TYPE="default"
fi

# =============================================================================
# Main Functions
# =============================================================================

check_cursor_settings_dependencies() {
    log "INFO" "Checking Cursor settings dependencies..."
    
    # Check if Cursor settings file exists
    if [[ ! -f "$CURSOR_SETTINGS_SOURCE" ]]; then
        log "ERROR" "Cursor settings file not found: $CURSOR_SETTINGS_SOURCE"
        log "INFO" "This might mean Cursor is not installed or has not been configured yet"
        exit 1
    fi
    
    log "SUCCESS" "Cursor settings file found ($SETTINGS_TYPE settings)"
    log "INFO" "Using settings from: $CURSOR_SETTINGS_SOURCE"
}

snapshot_cursor_settings() {
    log "STEP" "Starting Cursor settings snapshot..."
    
    # Backup existing file (only if backups are enabled)
    backup_file "$CURSOR_SETTINGS_FILE" "cursor_settings.json"
    
    # Remove existing file
    remove_file_if_exists "$CURSOR_SETTINGS_FILE" "cursor_settings.json"
    
    # Create new snapshot
    log "INFO" "Creating new Cursor settings snapshot..."
    log "INFO" "Source type: $SETTINGS_TYPE settings"
    log "INFO" "Copying: $CURSOR_SETTINGS_SOURCE -> $CURSOR_SETTINGS_FILE"
    
    if cp "$CURSOR_SETTINGS_SOURCE" "$CURSOR_SETTINGS_FILE"; then
        log "SUCCESS" "Cursor settings snapshot created successfully"
        
        # Get file size
        local file_size=$(wc -c < "$CURSOR_SETTINGS_FILE" 2>/dev/null || echo "0")
        log "INFO" "Settings file size: $file_size bytes"
        
        # Validate JSON format
        if command -v jq &> /dev/null; then
            if jq empty "$CURSOR_SETTINGS_FILE" 2>/dev/null; then
                log "SUCCESS" "Settings file is valid JSON"
            else
                log "WARNING" "Settings file may not be valid JSON (jq validation failed)"
            fi
        else
            log "INFO" "jq not available - skipping JSON validation"
        fi
    else
        log "ERROR" "Failed to create Cursor settings snapshot"
        return 1
    fi
}

validate_cursor_settings_snapshot() {
    log "INFO" "Validating Cursor settings snapshot..."
    
    if validate_file "$CURSOR_SETTINGS_FILE" "cursor_settings.json"; then
        log "SUCCESS" "Cursor settings snapshot validated"
        return 0
    else
        log "ERROR" "Cursor settings snapshot validation failed"
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
    init_snapshot "cursor-settings-snapshot.sh" "cursor-settings-snapshot.log" "$enable_backup" "$shared_timestamp"
    
    # Check dependencies
    check_cursor_settings_dependencies
    
    # Create snapshot
    if snapshot_cursor_settings; then
        log "SUCCESS" "Cursor settings snapshot completed"
        
        # Validate snapshot
        if validate_cursor_settings_snapshot; then
            log "SUCCESS" "Cursor settings snapshot process finished successfully"
            log "INFO" "File created: $CURSOR_SETTINGS_FILE"
        else
            log "ERROR" "Cursor settings snapshot validation failed"
            exit 1
        fi
    else
        log "ERROR" "Cursor settings snapshot failed"
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