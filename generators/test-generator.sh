#!/bin/bash

# =============================================================================
# Test Generator Snapshot Script
# =============================================================================
# This script creates a simple test snapshot that works on all platforms.
# Used for testing the DotSnapshot installation in Homebrew CI.

# Source common utilities
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$(dirname "$SCRIPT_DIR")/lib/common.sh"

# =============================================================================
# Configuration
# =============================================================================

# File paths - use test-specific directory
TEST_DIR="$SNAPSHOT_DIR/latest-test"
TEST_FILE="$TEST_DIR/test_snapshot.txt"

# =============================================================================
# Main Functions
# =============================================================================

check_test_dependencies() {
    log "INFO" "Checking test generator dependencies..."
    
    # This generator has no external dependencies
    log "SUCCESS" "All test generator dependencies are available"
}

snapshot_test_data() {
    log "STEP" "Starting test snapshot..."
    
    # Create test directory
    mkdir -p "$TEST_DIR"
    
    # Backup existing file (only if backups are enabled)
    backup_file "$TEST_FILE" "test_snapshot.txt"
    
    # Remove existing file
    remove_file_if_exists "$TEST_FILE" "test_snapshot.txt"
    
    # Create new snapshot with system information
    log "INFO" "Creating new test snapshot..."
    
    cat > "$TEST_FILE" << EOF
# Test Snapshot
# Generated on: $(date)
# Platform: $(uname)
# Architecture: $(uname -m)
# Hostname: $(hostname)
# User: $(whoami)
# Home directory: $HOME
# Current working directory: $(pwd)
# Bash version: $(bash --version | head -n1)
# DotSnapshot version: $(cat "$(dirname "$SCRIPT_DIR")/VERSION" 2>/dev/null || echo "unknown")

# System information:
$(uname -a)

# Environment variables:
PATH=$PATH
SHELL=$SHELL

# Test completed successfully!
EOF
    
    if [[ -f "$TEST_FILE" ]]; then
        log "SUCCESS" "Test snapshot created successfully"
        
        # Count lines
        local line_count=$(wc -l < "$TEST_FILE" 2>/dev/null || echo "0")
        log "INFO" "Test snapshot contains $line_count lines"
    else
        log "ERROR" "Failed to create test snapshot"
        return 1
    fi
}

validate_test_snapshot() {
    log "INFO" "Validating test snapshot..."
    
    if validate_file "$TEST_FILE" "test_snapshot.txt"; then
        log "SUCCESS" "Test snapshot validated"
        return 0
    else
        log "ERROR" "Test snapshot validation failed"
        return 1
    fi
}

cleanup_test_files() {
    log "INFO" "Cleaning up test files..."
    
    if [[ -d "$TEST_DIR" ]]; then
        if rm -rf "$TEST_DIR"; then
            log "SUCCESS" "Test directory cleaned up: $TEST_DIR"
        else
            log "WARNING" "Failed to clean up test directory: $TEST_DIR"
        fi
    else
        log "INFO" "Test directory does not exist, nothing to clean up"
    fi
}

# =============================================================================
# Main Execution
# =============================================================================

main() {
    # Initialize snapshot (backup disabled by default for individual scripts)
    local enable_backup="${1:-false}"
    local shared_timestamp="${2:-}"
    local enable_cleanup="${3:-true}"
    init_snapshot "test-generator-snapshot.sh" "test-generator-snapshot.log" "$enable_backup" "$shared_timestamp"
    
    # Check dependencies
    check_test_dependencies
    
    # Create snapshot
    if snapshot_test_data; then
        log "SUCCESS" "Test snapshot completed"
        
        # Validate snapshot
        if validate_test_snapshot; then
            log "SUCCESS" "Test snapshot process finished successfully"
            log "INFO" "File created: $TEST_FILE"
            
            # Clean up test files if enabled
            if [[ "$enable_cleanup" == "true" ]]; then
                cleanup_test_files
            else
                log "INFO" "Cleanup disabled, test files preserved"
            fi
        else
            log "ERROR" "Test snapshot validation failed"
            exit 1
        fi
    else
        log "ERROR" "Test snapshot failed"
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