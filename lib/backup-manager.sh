#!/bin/bash

# =============================================================================
# Backup Manager Script
# =============================================================================
# This script manages backup retention by removing backups older than 30 days.
# It should be called after snapshot operations to keep the backup directory clean.

# Source common utilities
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

# =============================================================================
# Configuration
# =============================================================================

# Retention settings (loaded from common.sh)
RETENTION_SECONDS=$((DSNP_BACKUP_RETENTION_DAYS * 24 * 60 * 60))

# =============================================================================
# Main Functions
# =============================================================================

cleanup_old_backups() {
    log "STEP" "Starting backup cleanup process..."
    log "INFO" "Retention period: $DSNP_BACKUP_RETENTION_DAYS days"
    log "INFO" "Backup directory: $BACKUP_DIR"
    
    # Check if backup directory exists
    if [[ ! -d "$BACKUP_DIR" ]]; then
        log "INFO" "Backup directory does not exist, nothing to clean"
        return 0
    fi
    
    # Get current timestamp
    local current_time=$(date +%s)
    local cutoff_time=$((current_time - RETENTION_SECONDS))
    local cutoff_date=$(date -r "$cutoff_time" '+%Y-%m-%d %H:%M:%S')
    
    log "INFO" "Current time: $(date '+%Y-%m-%d %H:%M:%S')"
    log "INFO" "Cutoff time: $cutoff_date (backups older than this will be removed)"
    
    # Find all backup directories
    local backup_dirs=()
    while IFS= read -r -d '' dir; do
        backup_dirs+=("$dir")
    done < <(find "$BACKUP_DIR" -maxdepth 1 -type d -name "*" -print0 2>/dev/null)
    
    local total_backups=${#backup_dirs[@]}
    local removed_count=0
    local kept_count=0
    
    log "INFO" "Found $total_backups backup directories to check"
    
    # Process each backup directory
    for backup_dir in "${backup_dirs[@]}"; do
        # Skip the backup directory itself
        if [[ "$backup_dir" == "$BACKUP_DIR" ]]; then
            continue
        fi
        
        local dir_name=$(basename "$backup_dir")
        local dir_time
        
        # Try to extract timestamp from directory name (format: YYYYMMDD_HHMMSS)
        if [[ "$dir_name" =~ ^[0-9]{8}_[0-9]{6}$ ]]; then
            # Convert directory name to timestamp
            local year=${dir_name:0:4}
            local month=${dir_name:4:2}
            local day=${dir_name:6:2}
            local hour=${dir_name:9:2}
            local minute=${dir_name:11:2}
            local second=${dir_name:13:2}
            
            # Create date string and convert to timestamp
            local date_string="$year-$month-$day $hour:$minute:$second"
            dir_time=$(date -j -f "%Y-%m-%d %H:%M:%S" "$date_string" +%s 2>/dev/null)
        else
            # Fallback: use directory modification time
            dir_time=$(stat -f %m "$backup_dir" 2>/dev/null || echo "0")
        fi
        
        if [[ "$dir_time" == "0" ]] || [[ -z "$dir_time" ]]; then
            log "WARNING" "Could not determine age of backup directory: $dir_name"
            kept_count=$((kept_count + 1))
            continue
        fi
        
        local dir_date=$(date -r "$dir_time" '+%Y-%m-%d %H:%M:%S')
        local age_seconds=$((current_time - dir_time))
        local age_days=$((age_seconds / 86400))
        
        if [[ "$dir_time" -lt "$cutoff_time" ]]; then
            log "INFO" "Removing old backup: $dir_name (age: ${age_days} days, created: $dir_date)"
            
            if rm -rf "$backup_dir"; then
                log "SUCCESS" "Successfully removed backup: $dir_name"
                removed_count=$((removed_count + 1))
            else
                log "ERROR" "Failed to remove backup: $dir_name"
            fi
        else
            log "INFO" "Keeping backup: $dir_name (age: ${age_days} days, created: $dir_date)"
            kept_count=$((kept_count + 1))
        fi
    done
    
    # Summary
    log "SUCCESS" "Backup cleanup completed"
    log "INFO" "Summary:"
    log "INFO" "  - Total backups checked: $total_backups"
    log "INFO" "  - Backups removed: $removed_count"
    log "INFO" "  - Backups kept: $kept_count"
    
    if [[ "$removed_count" -gt 0 ]]; then
        log "SUCCESS" "Cleaned up $removed_count old backup(s)"
    else
        log "INFO" "No old backups found to remove"
    fi
}

get_backup_stats() {
    log "INFO" "Getting backup statistics..."
    
    if [[ ! -d "$BACKUP_DIR" ]]; then
        log "INFO" "Backup directory does not exist"
        return 0
    fi
    
    # Count backup directories
    local backup_count=$(find "$BACKUP_DIR" -maxdepth 1 -type d | wc -l)
    backup_count=$((backup_count - 1))  # Subtract 1 for the backup directory itself
    
    # Calculate total size
    local total_size=$(du -sh "$BACKUP_DIR" 2>/dev/null | cut -f1)
    
    log "INFO" "Backup statistics for machine '$MACHINE_NAME':"
    log "INFO" "  - Total backup directories: $backup_count"
    log "INFO" "  - Total size: $total_size"
    
    # List recent backups
    log "INFO" "Recent backups:"
    find "$BACKUP_DIR" -maxdepth 1 -type d -name "*" -exec basename {} \; 2>/dev/null | \
        grep -E '^[0-9]{8}_[0-9]{6}$' | \
        sort -r | \
        head -5 | \
        while read -r backup; do
            local backup_path="$BACKUP_DIR/$backup"
            local backup_date=$(date -r "$(stat -f %m "$backup_path" 2>/dev/null || echo "0")" '+%Y-%m-%d %H:%M:%S' 2>/dev/null)
            local backup_size=$(du -sh "$backup_path" 2>/dev/null | cut -f1)
            log "INFO" "    - $backup ($backup_date, $backup_size)"
        done
}

# =============================================================================
# Main Execution
# =============================================================================

main() {
    # Initialize (no backup needed for this script)
    init_snapshot "backup-manager.sh" "backup-manager.log" "false"
    
    # Get backup statistics
    get_backup_stats
    
    # Clean up old backups
    cleanup_old_backups
    
    log "SUCCESS" "Backup management process completed"
}

# =============================================================================
# Script Execution
# =============================================================================

# Only run main if script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi