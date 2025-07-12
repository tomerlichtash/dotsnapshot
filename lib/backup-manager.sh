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

# =============================================================================
# process_backup_dir
# =============================================================================
# Processes a single backup directory by determining whether to keep or remove it
# based on its age compared to the retention period.
#
# Parameters:
#   $1 - backup_dir: Full path to the backup directory
#   $2 - dir_name: Name of the backup directory (basename)
#   $3 - dir_time: Unix timestamp of when the backup was created
#   $4 - cutoff_time: Unix timestamp cutoff (backups older than this will be removed)
#   $5 - current_time: Current Unix timestamp
#   $6 - removed_count_ref: Reference to variable tracking number of removed backups
#   $7 - kept_count_ref: Reference to variable tracking number of kept backups
#
# Returns: None (modifies referenced count variables)
# =============================================================================
process_backup_dir() {
    local backup_dir="$1"
    local dir_name="$2"
    local dir_time="$3"
    local cutoff_time="$4"
    local current_time="$5"
    local removed_count_ref="$6"
    local kept_count_ref="$7"

    local dir_date=$(date -r "$dir_time" '+%Y-%m-%d %H:%M:%S')
    local age_seconds=$((current_time - dir_time))
    local age_days=$((age_seconds / 86400))

    if [[ "$dir_time" -lt "$cutoff_time" ]]; then
        log "INFO" "Removing old backup: $dir_name (age: ${age_days} days, created: $dir_date)"
        if rm -rf "$backup_dir"; then
            log "SUCCESS" "Successfully removed backup: $dir_name"
            eval "$removed_count_ref=\$((\$$removed_count_ref + 1))"
        else
            log "ERROR" "Failed to remove backup: $dir_name"
        fi
    else
        log "INFO" "Keeping backup: $dir_name (age: ${age_days} days, created: $dir_date)"
        eval "$kept_count_ref=\$((\$$kept_count_ref + 1))"
    fi
}

# =============================================================================
# process_backup_directory
# =============================================================================
# Processes a single backup directory by extracting its timestamp and determining
# if it should be processed further. Handles edge cases like invalid timestamps.
#
# Parameters:
#   $1 - backup_dir: Full path to the backup directory
#   $2 - cutoff_time: Unix timestamp cutoff for retention
#   $3 - current_time: Current Unix timestamp
#   $4 - removed_count_ref: Reference to variable tracking removed backups count
#   $5 - kept_count_ref: Reference to variable tracking kept backups count
#
# Returns: None (modifies referenced count variables)
# =============================================================================
process_backup_directory() {
    local backup_dir="$1"
    local cutoff_time="$2"
    local current_time="$3"
    local removed_count_ref="$4"
    local kept_count_ref="$5"

    # Ensure BACKUP_DIR is set (source common.sh if not)
    if [[ -z "$BACKUP_DIR" ]]; then
        source "$(dirname "$0")/common.sh"
    fi

    # Skip the backup directory itself
    if [[ "$backup_dir" == "$BACKUP_DIR" ]]; then
        return 0
    fi
    
    local dir_name=$(basename "$backup_dir")
    local dir_time
    
    # Try to extract timestamp from directory name (format: YYYYMMDD_HHMMSS)
    dir_time=$(parse_timestamp_from_dirname "$dir_name")
    
    if [[ -z "$dir_time" ]]; then
        # Fallback: use directory modification time
        dir_time=$(stat -f %m "$backup_dir" 2>/dev/null || echo "0")
    fi
    
    if [[ "$dir_time" == "0" ]] || [[ -z "$dir_time" ]]; then
        log "WARNING" "Could not determine age of backup directory: $dir_name"
        eval "$kept_count_ref=\$((\$$kept_count_ref + 1))"
        return 0
    fi
    
    process_backup_dir "$backup_dir" "$dir_name" "$dir_time" "$cutoff_time" "$current_time" "$removed_count_ref" "$kept_count_ref"
}

# =============================================================================
# cleanup_old_backups
# =============================================================================
# Main function that cleans up old backup directories based on the configured
# retention period. Finds all backup directories, determines their age, and
# removes those that exceed the retention period.
#
# Parameters: None
# Returns: None
# Side Effects: Removes old backup directories, logs cleanup results
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
        process_backup_directory "$backup_dir" "$cutoff_time" "$current_time" removed_count kept_count
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

# =============================================================================
# list_recent_backups
# =============================================================================
# Lists the most recent backup directories with their creation dates and sizes.
# Useful for providing an overview of available backups.
#
# Parameters:
#   $1 - backup_dir: Directory containing backup folders
#   $2 - max_count: Maximum number of recent backups to list (default: 5)
#
# Returns: None (outputs to log)
# =============================================================================
list_recent_backups() {
    local backup_dir="$1"
    local max_count="${2:-5}"
    
    if [[ ! -d "$backup_dir" ]]; then
        log "INFO" "Backup directory does not exist: $backup_dir"
        return 0
    fi
    
    log "INFO" "Recent backups:"
    find "$backup_dir" -maxdepth 1 -type d -name "*" -exec basename {} \; 2>/dev/null | \
        grep -E '^[0-9]{8}_[0-9]{6}$' | \
        sort -r | \
        head -"$max_count" | \
        while read -r backup; do
            local backup_path="$backup_dir/$backup"
            local backup_date=$(date -r "$(stat -f %m "$backup_path" 2>/dev/null || echo "0")" '+%Y-%m-%d %H:%M:%S' 2>/dev/null)
            local backup_size=$(du -sh "$backup_path" 2>/dev/null | cut -f1)
            log "INFO" "    - $backup ($backup_date, $backup_size)"
        done
}

# =============================================================================
# get_backup_stats
# =============================================================================
# Retrieves and displays statistics about backup directories including total
# count, size, and lists recent backups. Provides an overview of backup status.
#
# Parameters: None
# Returns: None (outputs statistics to log)
# =============================================================================
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
    list_recent_backups "$BACKUP_DIR" 5
}

# =============================================================================
# Main Execution
# =============================================================================

# =============================================================================
# main
# =============================================================================
# Main entry point for the backup manager script. Orchestrates the backup
# management workflow by getting statistics and cleaning up old backups.
# This function is called when the script is executed directly.
#
# Parameters: None
# Returns: None
# Side Effects: 
#   - Logs backup statistics
#   - Removes old backup directories
#   - Creates log files
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