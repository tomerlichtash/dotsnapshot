#!/bin/bash

# =============================================================================
# Common Snapshot Utilities
# =============================================================================
# This file contains shared functionality for all snapshot scripts:
# - Logging functions
# - Directory management
# - Backup functionality
# - Common configuration

set -euo pipefail

# =============================================================================
# Configuration
# =============================================================================

# Script configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# =============================================================================
# Configuration Loading
# =============================================================================

# Load configuration file
# Check for Homebrew configuration first
if [[ -f "/usr/local/etc/dotsnapshot/dotsnapshot.conf" ]]; then
    CONFIG_FILE="/usr/local/etc/dotsnapshot/dotsnapshot.conf"
elif [[ -f "/opt/homebrew/etc/dotsnapshot/dotsnapshot.conf" ]]; then
    CONFIG_FILE="/opt/homebrew/etc/dotsnapshot/dotsnapshot.conf"
elif [[ -f "$PROJECT_ROOT/config/dotsnapshot.conf" ]]; then
    CONFIG_FILE="$PROJECT_ROOT/config/dotsnapshot.conf"
else
    CONFIG_FILE="$PROJECT_ROOT/config/dotsnapshot.conf"
fi
if [[ -f "$CONFIG_FILE" ]]; then
    source "$CONFIG_FILE"
else
    # Default values if config file doesn't exist
    DSNP_SNAPSHOT_TARGET_DIR=".snapshot"
    DSNP_BACKUP_RETENTION_DAYS=30
    DSNP_LOGS_DIR=".logs"
    DSNP_USE_MACHINE_DIRECTORIES=true
fi

# Allow environment variable overrides for all config options
if [[ -n "${DSNP_SNAPSHOT_TARGET_DIR_ENV:-}" ]]; then
    DSNP_SNAPSHOT_TARGET_DIR="$DSNP_SNAPSHOT_TARGET_DIR_ENV"
fi

if [[ -n "${DSNP_BACKUP_RETENTION_DAYS_ENV:-}" ]]; then
    DSNP_BACKUP_RETENTION_DAYS="$DSNP_BACKUP_RETENTION_DAYS_ENV"
fi

if [[ -n "${DSNP_LOGS_DIR_ENV:-}" ]]; then
    DSNP_LOGS_DIR="$DSNP_LOGS_DIR_ENV"
fi

if [[ -n "${DSNP_USE_MACHINE_DIRECTORIES_ENV:-}" ]]; then
    DSNP_USE_MACHINE_DIRECTORIES="$DSNP_USE_MACHINE_DIRECTORIES_ENV"
fi

# Get machine name (hostname)
MACHINE_NAME=$(hostname 2>/dev/null || echo "unknown-machine")

# Directory structure with configurable target and machine name
# Check if DSNP_SNAPSHOT_TARGET_DIR is an absolute path
if [[ "$DSNP_SNAPSHOT_TARGET_DIR" = /* ]]; then
    # Absolute path - use as is
    SNAPSHOT_DIR="$DSNP_SNAPSHOT_TARGET_DIR"
else
    # Relative path - prepend project root
    SNAPSHOT_DIR="$PROJECT_ROOT/$DSNP_SNAPSHOT_TARGET_DIR"
fi

# Use machine-specific directories if enabled
if [[ "${DSNP_USE_MACHINE_DIRECTORIES:-true}" == "true" ]]; then
    MACHINE_SNAPSHOT_DIR="$SNAPSHOT_DIR/$MACHINE_NAME"
    LATEST_DIR="$MACHINE_SNAPSHOT_DIR/latest"
    BACKUP_DIR="$MACHINE_SNAPSHOT_DIR/backups"
else
    MACHINE_SNAPSHOT_DIR="$SNAPSHOT_DIR"
    LATEST_DIR="$SNAPSHOT_DIR/latest"
    BACKUP_DIR="$SNAPSHOT_DIR/backups"
fi

# Check if DSNP_LOGS_DIR is an absolute path
if [[ "$DSNP_LOGS_DIR" = /* ]]; then
    # Absolute path - use as is
    :
else
    # Relative path - prepend project root
    DSNP_LOGS_DIR="$PROJECT_ROOT/$DSNP_LOGS_DIR"
fi

# Global variables
RUN_TIMESTAMP=""
SHOULD_BACKUP=false

# Colors for logging
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

# =============================================================================
# Logging Functions
# =============================================================================

# =============================================================================
# log
# =============================================================================
# Centralized logging function that outputs colored messages to both console
# and log file. Supports different log levels with appropriate colors.
#
# Parameters:
#   $1 - level: Log level (INFO, SUCCESS, WARNING, ERROR, STEP, or custom)
#   $2 - message: The message to log
#
# Returns: None (outputs to console and log file)
# Side Effects: Creates logs directory if it doesn't exist
# =============================================================================
log() {
    local level="$1"
    local message="$2"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    
    # Ensure logs directory exists before writing
    if [[ ! -d "$DSNP_LOGS_DIR" ]]; then
        mkdir -p "$DSNP_LOGS_DIR"
    fi
    
    case "$level" in
        "INFO")
            echo -e "${BLUE}[${timestamp}] [INFO]${NC} $message" | tee -a "$LOG_FILE"
            ;;
        "SUCCESS")
            echo -e "${GREEN}[${timestamp}] [SUCCESS]${NC} $message" | tee -a "$LOG_FILE"
            ;;
        "WARNING")
            echo -e "${YELLOW}[${timestamp}] [WARNING]${NC} $message" | tee -a "$LOG_FILE"
            ;;
        "ERROR")
            echo -e "${RED}[${timestamp}] [ERROR]${NC} $message" | tee -a "$LOG_FILE"
            ;;
        "STEP")
            echo -e "${PURPLE}[${timestamp}] [STEP]${NC} $message" | tee -a "$LOG_FILE"
            ;;
        *)
            echo -e "[${timestamp}] [$level] $message" | tee -a "$LOG_FILE"
            ;;
    esac
}

# =============================================================================
# Directory Management
# =============================================================================

# =============================================================================
# ensure_directories
# =============================================================================
# Creates all required directories for the snapshot system if they don't exist.
# This includes snapshot directories, machine-specific directories (if enabled),
# latest directory, backup directory (if backups enabled), and logs directory.
#
# Parameters: None
# Returns: None
# Side Effects: Creates directories as needed
# =============================================================================
ensure_directories() {
    log "INFO" "Ensuring required directories exist..."
    
    # Ensure snapshot directory exists
    if [[ ! -d "$SNAPSHOT_DIR" ]]; then
        log "INFO" "Creating snapshot directory: $SNAPSHOT_DIR"
        mkdir -p "$SNAPSHOT_DIR"
        log "SUCCESS" "Snapshot directory created"
    else
        log "INFO" "Snapshot directory already exists: $SNAPSHOT_DIR"
    fi
    
    # Ensure machine-specific snapshot directory exists (if enabled)
    if [[ "${DSNP_USE_MACHINE_DIRECTORIES:-true}" == "true" ]]; then
        if [[ ! -d "$MACHINE_SNAPSHOT_DIR" ]]; then
            log "INFO" "Creating machine snapshot directory: $MACHINE_SNAPSHOT_DIR"
            mkdir -p "$MACHINE_SNAPSHOT_DIR"
            log "SUCCESS" "Machine snapshot directory created"
        else
            log "INFO" "Machine snapshot directory already exists: $MACHINE_SNAPSHOT_DIR"
        fi
    fi
    
    # Ensure latest directory exists
    if [[ ! -d "$LATEST_DIR" ]]; then
        log "INFO" "Creating latest directory: $LATEST_DIR"
        mkdir -p "$LATEST_DIR"
        log "SUCCESS" "Latest directory created"
    else
        log "INFO" "Latest directory already exists: $LATEST_DIR"
    fi
    
    # Ensure backup directory exists (only if backups are enabled)
    if [[ "$SHOULD_BACKUP" == "true" ]]; then
        if [[ ! -d "$BACKUP_DIR" ]]; then
            log "INFO" "Creating backup directory: $BACKUP_DIR"
            mkdir -p "$BACKUP_DIR"
            log "SUCCESS" "Backup directory created"
        else
            log "INFO" "Backup directory already exists: $BACKUP_DIR"
        fi
    fi
    
    # Ensure logs directory exists
    if [[ ! -d "$DSNP_LOGS_DIR" ]]; then
        log "INFO" "Creating logs directory: $DSNP_LOGS_DIR"
        mkdir -p "$DSNP_LOGS_DIR"
        log "SUCCESS" "Logs directory created"
    else
        log "INFO" "Logs directory already exists: $DSNP_LOGS_DIR"
    fi
}

# =============================================================================
# Backup Functions
# =============================================================================

# =============================================================================
# backup_file
# =============================================================================
# Creates a backup of an existing file before it's overwritten. Only creates
# backups if backup functionality is enabled. Backups are stored in timestamped
# directories for organization.
#
# Parameters:
#   $1 - file_path: Full path to the file to backup
#   $2 - file_name: Display name of the file (for logging purposes)
#
# Returns: None
# Side Effects: Creates backup file in backup directory
# =============================================================================
backup_file() {
    local file_path="$1"
    local file_name="$2"
    
    # Only backup if backups are enabled
    if [[ "$SHOULD_BACKUP" != "true" ]]; then
        return 0
    fi
    
    if [[ -f "$file_path" ]]; then
        local backup_run_dir="$BACKUP_DIR/${RUN_TIMESTAMP}"
        local backup_path="$backup_run_dir/$file_name"
        
        # Create backup run directory
        mkdir -p "$backup_run_dir"
        
        log "INFO" "Backing up existing $file_name to: $backup_path"
        cp "$file_path" "$backup_path"
        log "SUCCESS" "Backup created successfully in: $backup_run_dir"
    fi
}

# =============================================================================
# Dependency Checking
# =============================================================================

# =============================================================================
# check_dependency
# =============================================================================
# Checks if a required command is available in the system PATH. Used to
# validate that external dependencies are installed before running operations
# that require them.
#
# Parameters:
#   $1 - dependency: Human-readable name of the dependency
#   $2 - command_name: The actual command to check in PATH
#
# Returns: 0 if dependency is available, 1 if missing
# Side Effects: Logs dependency status
# =============================================================================
check_dependency() {
    local dependency="$1"
    local command_name="$2"
    
    if ! command -v "$command_name" &> /dev/null; then
        log "ERROR" "Missing dependency: $dependency ($command_name)"
        return 1
    fi
    
    log "SUCCESS" "Dependency $dependency is available"
    return 0
}

# =============================================================================
# File Management
# =============================================================================

# =============================================================================
# remove_file_if_exists
# =============================================================================
# Safely removes a file if it exists, with appropriate logging. Does nothing
# if the file doesn't exist, preventing errors from missing files.
#
# Parameters:
#   $1 - file_path: Full path to the file to remove
#   $2 - file_name: Display name of the file (for logging purposes)
#
# Returns: None
# Side Effects: Removes file if it exists
# =============================================================================
remove_file_if_exists() {
    local file_path="$1"
    local file_name="$2"
    
    if [[ -f "$file_path" ]]; then
        log "INFO" "Removing existing $file_name"
        rm "$file_path"
    fi
}

# =============================================================================
# validate_file
# =============================================================================
# Validates that a file was created successfully and has content. Checks for
# file existence and non-empty status, providing appropriate feedback.
#
# Parameters:
#   $1 - file_path: Full path to the file to validate
#   $2 - file_name: Display name of the file (for logging purposes)
#
# Returns: 0 if file is valid, 1 if file is missing
# Side Effects: Logs validation results
# =============================================================================
validate_file() {
    local file_path="$1"
    local file_name="$2"
    
    if [[ ! -f "$file_path" ]]; then
        log "ERROR" "$file_name was not created"
        return 1
    elif [[ ! -s "$file_path" ]]; then
        log "WARNING" "$file_name is empty"
        return 0
    else
        log "SUCCESS" "$file_name validated successfully"
        return 0
    fi
}

# =============================================================================
# Utility Functions
# =============================================================================

# =============================================================================
# parse_timestamp_from_dirname
# =============================================================================
# Extracts a Unix timestamp from a directory name that follows the format
# YYYYMMDD_HHMMSS. Used for parsing backup directory names to determine
# their age for cleanup operations.
#
# Parameters:
#   $1 - dir_name: Directory name in format YYYYMMDD_HHMMSS
#
# Returns: Unix timestamp as string on success, empty string on failure
# Exit Code: 0 on success, 1 on failure
# =============================================================================
parse_timestamp_from_dirname() {
    local dir_name="$1"
    
    # Check if directory name matches timestamp format (YYYYMMDD_HHMMSS)
    if [[ "$dir_name" =~ ^[0-9]{8}_[0-9]{6}$ ]]; then
        # Extract timestamp components
        local year=${dir_name:0:4}
        local month=${dir_name:4:2}
        local day=${dir_name:6:2}
        local hour=${dir_name:9:2}
        local minute=${dir_name:11:2}
        local second=${dir_name:13:2}
        
        # Create date string and convert to timestamp
        local date_string="$year-$month-$day $hour:$minute:$second"
        local timestamp=$(date -j -f "%Y-%m-%d %H:%M:%S" "$date_string" +%s 2>/dev/null)
        
        if [[ -n "$timestamp" ]] && [[ "$timestamp" != "0" ]]; then
            echo "$timestamp"
            return 0
        fi
    fi
    
    # Return empty string if parsing failed
    echo ""
    return 1
}

# =============================================================================
# Initialization
# =============================================================================

# =============================================================================
# init_snapshot
# =============================================================================
# Initializes the snapshot system by setting up global variables, creating
# timestamps, and ensuring all required directories exist. This function
# must be called at the beginning of any snapshot script.
#
# Parameters:
#   $1 - script_name: Name of the calling script (for logging)
#   $2 - log_file_name: Name of the log file to create
#   $3 - enable_backup: Whether to enable backup functionality (default: false)
#   $4 - external_timestamp: Optional external timestamp to use (default: auto-generated)
#
# Returns: None
# Side Effects: 
#   - Sets global variables (RUN_TIMESTAMP, SHOULD_BACKUP, LOG_FILE)
#   - Creates required directories
#   - Logs initialization information
# =============================================================================
init_snapshot() {
    local script_name="$1"
    local log_file_name="$2"
    local enable_backup="${3:-false}"
    local external_timestamp="${4:-}"
    
    # Set global variables
    if [[ -n "$external_timestamp" ]]; then
        RUN_TIMESTAMP="$external_timestamp"
    else
        RUN_TIMESTAMP=$(date +%Y%m%d_%H%M%S)
    fi
    SHOULD_BACKUP="$enable_backup"
    LOG_FILE="$DSNP_LOGS_DIR/$log_file_name"
    
    log "INFO" "Starting $script_name..."
    log "INFO" "Script: $script_name"
    log "INFO" "Project root: $PROJECT_ROOT"
    log "INFO" "Machine name: $MACHINE_NAME"
    log "INFO" "Snapshot target directory: $DSNP_SNAPSHOT_TARGET_DIR"
    if [[ -n "${DSNP_SNAPSHOT_TARGET_DIR_ENV:-}" ]]; then
        log "INFO" "  (overridden by DSNP_SNAPSHOT_TARGET_DIR_ENV environment variable)"
    fi
    log "INFO" "Backup retention period: $DSNP_BACKUP_RETENTION_DAYS days"
    if [[ -n "${DSNP_BACKUP_RETENTION_DAYS_ENV:-}" ]]; then
        log "INFO" "  (overridden by DSNP_BACKUP_RETENTION_DAYS_ENV environment variable)"
    fi
    log "INFO" "Logs directory: $DSNP_LOGS_DIR"
    if [[ -n "${DSNP_LOGS_DIR_ENV:-}" ]]; then
        log "INFO" "  (overridden by DSNP_LOGS_DIR_ENV environment variable)"
    fi
    log "INFO" "Use machine directories: $DSNP_USE_MACHINE_DIRECTORIES"
    if [[ -n "${DSNP_USE_MACHINE_DIRECTORIES_ENV:-}" ]]; then
        log "INFO" "  (overridden by DSNP_USE_MACHINE_DIRECTORIES_ENV environment variable)"
    fi
    log "INFO" "Snapshot directory: $SNAPSHOT_DIR"
    if [[ "${DSNP_USE_MACHINE_DIRECTORIES:-true}" == "true" ]]; then
        log "INFO" "Machine snapshot directory: $MACHINE_SNAPSHOT_DIR"
    fi
    log "INFO" "Latest directory: $LATEST_DIR"
    log "INFO" "Logs directory: $DSNP_LOGS_DIR"
    log "INFO" "Log file: $LOG_FILE"
    
    if [[ "$SHOULD_BACKUP" == "true" ]]; then
        log "INFO" "Backup directory: $BACKUP_DIR"
        log "INFO" "Backup run ID: ${RUN_TIMESTAMP}"
    fi
    
    # Ensure directories exist
    ensure_directories
}