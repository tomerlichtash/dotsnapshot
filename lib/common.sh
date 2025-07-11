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
    DSNP_LOGS_DIR="$DSNP_LOGS_DIR"
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

remove_file_if_exists() {
    local file_path="$1"
    local file_name="$2"
    
    if [[ -f "$file_path" ]]; then
        log "INFO" "Removing existing $file_name"
        rm "$file_path"
    fi
}

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
# Initialization
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