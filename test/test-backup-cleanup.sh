#!/bin/bash

# =============================================================================
# Test Backup Cleanup Script
# =============================================================================
# This script creates mock backup directories to test the backup cleanup functionality.
# It creates some recent backups and some old backups (>30 days) to demonstrate cleanup.

# Source common utilities
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

# =============================================================================
# Test Functions
# =============================================================================

create_test_backups() {
    log "STEP" "Creating test backup directories..."
    
    # Create backup directory if it doesn't exist
    mkdir -p "$BACKUP_DIR"
    
    # Define test backup directories
    local test_backups=(
        # Old backups (should be removed by cleanup)
        "20240401_120000"  # April 1, 2024
        "20240515_120000"  # May 15, 2024
        "20240601_120000"  # June 1, 2024
        # Recent backups (should be kept by cleanup)
        "20250711_120000"  # July 11, 2025 12:00
        "20250711_130000"  # July 11, 2025 13:00
        "20250711_140000"  # July 11, 2025 14:00
    )
    
    # Create some recent backups (should be kept)
    log "INFO" "Creating recent backup directories..."
    for backup in "${test_backups[@]:3:3}"; do  # Recent backups (indices 3-5)
        mkdir -p "$BACKUP_DIR/$backup"
        # Add test files only to mock backups
        echo "test file" > "$BACKUP_DIR/$backup/test.txt"
        echo "backup data" > "$BACKUP_DIR/$backup/backup.json"
    done
    
    # Create some old backups (should be removed)
    log "INFO" "Creating old backup directories (>30 days)..."
    for backup in "${test_backups[@]:0:3}"; do  # Old backups (indices 0-2)
        mkdir -p "$BACKUP_DIR/$backup"
        # Add test files only to mock backups
        echo "test file" > "$BACKUP_DIR/$backup/test.txt"
        echo "backup data" > "$BACKUP_DIR/$backup/backup.json"
    done
    
    log "SUCCESS" "Created test backup directories"
}

show_backup_status() {
    log "INFO" "Backup summary:"
    if [[ -d "$BACKUP_DIR" ]]; then
        local count=$(find "$BACKUP_DIR" -mindepth 1 -maxdepth 1 -type d | wc -l | xargs)
        local size=$(du -sh "$BACKUP_DIR" 2>/dev/null | cut -f1)
        log "INFO" "  - Total backup directories: $count"
        log "INFO" "  - Total size: $size"
    else
        log "INFO" "  No backup directory found"
    fi
}

cleanup_test_backups() {
    log "STEP" "Cleaning up test backup directories..."
    local test_dirs=(
        # Old backups (should be removed by cleanup)
        "20240401_120000"  # April 1, 2024
        "20240515_120000"  # May 15, 2024
        "20240601_120000"  # June 1, 2024
        # Recent backups (should be kept by cleanup)
        "20250711_120000"  # July 11, 2025 12:00
        "20250711_130000"  # July 11, 2025 13:00
        "20250711_140000"  # July 11, 2025 14:00
    )
    for dir in "${test_dirs[@]}"; do
        local path="$BACKUP_DIR/$dir"
        if [[ -d "$path" ]]; then
            rm -rf "$path"
            log "INFO" "Removed test backup directory: $dir"
        fi
    done
    log "SUCCESS" "Test backup directories cleaned up"
}

# =============================================================================
# Main Execution
# =============================================================================

main() {
    # Initialize
    init_snapshot "test-backup-cleanup.sh" "test-backup-cleanup.log" "false"
    
    log "STEP" "Starting backup cleanup test..."
    
    # Show initial status
    log "INFO" "Initial backup status:"
    show_backup_status
    
    # Create test backups
    create_test_backups
    
    # Show status after creating test backups
    log "INFO" "Status after creating test backups:"
    show_backup_status
    
    # Run backup cleanup
    log "STEP" "Running backup cleanup..."
    if "$SCRIPT_DIR/backup-manager.sh"; then
        log "SUCCESS" "Backup cleanup completed successfully"
    else
        log "ERROR" "Backup cleanup failed"
        return 1
    fi
    
    # Show final status
    log "INFO" "Final backup status:"
    show_backup_status

    # Cleanup test backups
    cleanup_test_backups

    log "SUCCESS" "Backup cleanup test completed"
}

# =============================================================================
# Script Execution
# =============================================================================

# Only run main if script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi