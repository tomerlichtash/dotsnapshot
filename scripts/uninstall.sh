#!/bin/bash

# =============================================================================
# DotSnapshot Uninstall Script
# =============================================================================
# This script removes DotSnapshot from the system.
# 
# Usage:
#   ./scripts/uninstall.sh [OPTIONS]
# 
# Options:
#   --prefix DIR     Installation prefix (default: /usr/local)
#   --user           Uninstall user installation
#   --help           Show this help message

set -euo pipefail

# Script configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default uninstall settings
INSTALL_PREFIX="/usr/local"
UNINSTALL_FROM_USER=false
BIN_DIR=""
SHARE_DIR=""

# =============================================================================
# Utility Functions
# =============================================================================

log() {
    local level="$1"
    local message="$2"
    
    case "$level" in
        "INFO")
            echo -e "${BLUE}[INFO]${NC} $message"
            ;;
        "SUCCESS")
            echo -e "${GREEN}[SUCCESS]${NC} $message"
            ;;
        "WARNING")
            echo -e "${YELLOW}[WARNING]${NC} $message"
            ;;
        "ERROR")
            echo -e "${RED}[ERROR]${NC} $message"
            ;;
        *)
            echo "[$level] $message"
            ;;
    esac
}

show_help() {
    echo "DotSnapshot Uninstall Script"
    echo "============================"
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --prefix DIR     Installation prefix (default: /usr/local)"
    echo "  --user           Uninstall user installation"
    echo "  --help           Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                    # Uninstall system-wide installation"
    echo "  $0 --prefix /opt      # Uninstall from /opt"
    echo "  $0 --user             # Uninstall user installation"
    echo ""
    echo "This will remove:"
    echo "  - dotsnapshot executable"
    echo "  - All DotSnapshot files"
    echo "  - Installation directories (if empty)"
}

parse_arguments() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --prefix)
                if [[ -z "${2:-}" ]]; then
                    log "ERROR" "Prefix directory required"
                    exit 1
                fi
                INSTALL_PREFIX="$2"
                shift 2
                ;;
            --user)
                UNINSTALL_FROM_USER=true
                shift
                ;;
            --help|-h)
                show_help
                exit 0
                ;;
            *)
                log "ERROR" "Unknown option: $1"
                echo ""
                show_help
                exit 1
                ;;
        esac
    done
}

setup_uninstall_dirs() {
    if [[ "$UNINSTALL_FROM_USER" == "true" ]]; then
        BIN_DIR="$HOME/.local/bin"
        SHARE_DIR="$HOME/.local/share/dotsnapshot"
    else
        BIN_DIR="$INSTALL_PREFIX/bin"
        SHARE_DIR="$INSTALL_PREFIX/share/dotsnapshot"
    fi
}

confirm_uninstall() {
    echo "This will remove DotSnapshot from:"
    echo "  Executable: $BIN_DIR/dotsnapshot"
    echo "  Files:      $SHARE_DIR"
    echo ""
    
    read -p "Are you sure you want to continue? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        log "INFO" "Uninstall cancelled"
        exit 0
    fi
}

remove_executable() {
    local executable="$BIN_DIR/dotsnapshot"
    
    if [[ -f "$executable" ]]; then
        log "INFO" "Removing executable: $executable"
        rm -f "$executable"
        log "SUCCESS" "Executable removed"
    else
        log "WARNING" "Executable not found: $executable"
    fi
}

remove_share_directory() {
    if [[ -d "$SHARE_DIR" ]]; then
        log "INFO" "Removing share directory: $SHARE_DIR"
        rm -rf "$SHARE_DIR"
        log "SUCCESS" "Share directory removed"
    else
        log "WARNING" "Share directory not found: $SHARE_DIR"
    fi
}

cleanup_empty_directories() {
    # Remove empty parent directories
    local parent_dir=$(dirname "$SHARE_DIR")
    
    if [[ -d "$parent_dir" ]] && [[ -z "$(ls -A "$parent_dir" 2>/dev/null)" ]]; then
        log "INFO" "Removing empty parent directory: $parent_dir"
        rmdir "$parent_dir"
        log "SUCCESS" "Empty parent directory removed"
    fi
}

check_remaining_installations() {
    log "INFO" "Checking for other DotSnapshot installations..."
    
    local found_installations=false
    
    # Check common locations
    local common_locations=(
        "$HOME/.local/bin/dotsnapshot"
        "/usr/local/bin/dotsnapshot"
        "/usr/bin/dotsnapshot"
        "/opt/dotsnapshot/bin/dotsnapshot"
    )
    
    for location in "${common_locations[@]}"; do
        if [[ -f "$location" ]]; then
            log "WARNING" "Found another installation: $location"
            found_installations=true
        fi
    done
    
    if [[ "$found_installations" == "false" ]]; then
        log "SUCCESS" "No other DotSnapshot installations found"
    fi
}

show_uninstall_complete() {
    echo ""
    echo "Uninstall Complete!"
    echo "=================="
    echo ""
    echo "DotSnapshot has been removed from:"
    echo "  Executable: $BIN_DIR/dotsnapshot"
    echo "  Files:      $SHARE_DIR"
    echo ""
    echo "If you had added $BIN_DIR to your PATH, you can remove it now."
    echo ""
    echo "To reinstall DotSnapshot, run:"
    echo "  ./scripts/install.sh"
}

# =============================================================================
# Main Execution
# =============================================================================

main() {
    log "INFO" "Starting DotSnapshot uninstall..."
    
    # Parse command line arguments
    parse_arguments "$@"
    
    # Setup uninstall directories
    setup_uninstall_dirs
    
    log "INFO" "Uninstall prefix: $INSTALL_PREFIX"
    log "INFO" "Bin directory: $BIN_DIR"
    log "INFO" "Share directory: $SHARE_DIR"
    echo ""
    
    # Confirm uninstall
    confirm_uninstall
    
    # Remove files
    remove_executable
    remove_share_directory
    
    # Cleanup empty directories
    cleanup_empty_directories
    
    # Check for other installations
    check_remaining_installations
    
    # Show completion message
    show_uninstall_complete
}

# =============================================================================
# Script Execution
# =============================================================================

# Only run main if script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi 