#!/bin/bash

# =============================================================================
# DotSnapshot - Complete Snapshot Script (Main Orchestrator)
# =============================================================================
# This script orchestrates the creation of all snapshots or individual snapshots.
# 
# Usage:
#   ./dotsnapshot.sh                             # Run all snapshots with backups
#   ./dotsnapshot.sh generators/cursor-extensions.sh  # Run only cursor extensions snapshot (no backup)
#   ./dotsnapshot.sh generators/homebrew.sh      # Run only homebrew snapshot (no backup)
#   ./dotsnapshot.sh --list                      # List all available generators
#   ./dotsnapshot.sh --help                      # Show help

set -euo pipefail

# Script configuration
SCRIPT_NAME="dotsnapshot.sh"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SNAPSHOT_SCRIPTS_DIR="$SCRIPT_DIR"

# Get machine name (hostname)
MACHINE_NAME=$(hostname 2>/dev/null || echo "unknown-machine")

# Source configuration
source "$SNAPSHOT_SCRIPTS_DIR/lib/config.sh"

# =============================================================================
# Utility Functions
# =============================================================================

show_help() {
    echo "Usage: $SCRIPT_NAME [OPTIONS] [GENERATOR]"
    echo ""
    echo "Options:"
    echo "  --list, -l     List all available snapshot generators"
    echo "  --help, -h     Show this help message"
    echo ""
    echo "Arguments:"
echo "  GENERATOR      Name of specific snapshot generator to run"
echo "                 (e.g., generators/cursor-extensions.sh, generators/homebrew.sh)"
    echo ""
    echo "Examples:"
echo "  $SCRIPT_NAME                                    # Run all snapshots with backups"
echo "  $SCRIPT_NAME generators/cursor-extensions.sh    # Run only cursor extensions snapshot (no backup)"
echo "  $SCRIPT_NAME generators/homebrew.sh             # Run only homebrew snapshot (no backup)"
echo "  $SCRIPT_NAME --list                             # List all available generators"
    echo ""
    echo "When no GENERATOR is specified, all snapshots are run with backups enabled."
    echo "When a specific GENERATOR is specified, only that snapshot runs without backups."
}

run_snapshot_generator() {
    local generator_name="$1"
    local enable_backup="${2:-false}"
    local shared_timestamp="${3:-}"
    
    # Validate generator
    if ! is_valid_generator "$generator_name"; then
        echo "ERROR: Invalid snapshot generator: $generator_name"
        echo ""
        list_generators
        exit 1
    fi
    
    # Get display name
    local display_name
    display_name=$(get_display_name "$generator_name")
    
    # Check if script exists
    local script_path="$SNAPSHOT_SCRIPTS_DIR/$generator_name"
    if [[ ! -f "$script_path" ]]; then
        echo "ERROR: Snapshot script not found: $script_path"
        exit 1
    fi
    
    # Check if script is executable
    if [[ ! -x "$script_path" ]]; then
        echo "ERROR: Snapshot script is not executable: $script_path"
        exit 1
    fi
    
    echo "Running $display_name snapshot..."
    if "$script_path" "$enable_backup" "$shared_timestamp"; then
        echo "SUCCESS: $display_name snapshot completed"
        return 0
    else
        echo "ERROR: $display_name snapshot failed"
        return 1
    fi
}

run_all_snapshots() {
    echo "Starting complete snapshot process..."
    echo "Script: $SCRIPT_NAME"
    echo "Snapshot scripts directory: $SNAPSHOT_SCRIPTS_DIR"
    echo ""
    
    # Check if snapshot scripts directory exists
    if [[ ! -d "$SNAPSHOT_SCRIPTS_DIR" ]]; then
        echo "ERROR: Snapshot scripts directory not found: $SNAPSHOT_SCRIPTS_DIR"
        exit 1
    fi
    
    # Generate a single timestamp for this run
    local shared_timestamp=$(date +%Y%m%d_%H%M%S)
    echo "Backup run ID: $shared_timestamp"
    echo ""
    
    # Get all generators
    local generators
    read -ra generators <<< "$(get_snapshot_generators)"
    
    # Run each generator with backups enabled and shared timestamp
    local overall_success=true
    
    for generator in "${generators[@]}"; do
        if run_snapshot_generator "$generator" "true" "$shared_timestamp"; then
            echo ""
        else
            overall_success=false
            break
        fi
    done
    
    if [[ "$overall_success" == "true" ]]; then
        echo ""
        echo "SUCCESS: Complete snapshot process finished successfully"
        echo ""
        echo "Files created:"
        
        # Dynamically list created files based on generators
        for generator in "${generators[@]}"; do
            local display_name
            display_name=$(get_display_name "$generator")
            
            # Map generator names to their output files
            case "$generator" in
                "generators/cursor-extensions.sh")
                    echo "  - Cursor extensions: .snapshots/$MACHINE_NAME/latest/cursor_extensions"
                    ;;
                "generators/homebrew.sh")
                    echo "  - Brewfile: .snapshots/$MACHINE_NAME/latest/Brewfile"
                    ;;
                "generators/cursor-settings.sh")
                    echo "  - Cursor settings: .snapshots/$MACHINE_NAME/latest/cursor_settings.json"
                    ;;
                "generators/vscode-settings.sh")
                    echo "  - VS Code settings: .snapshots/$MACHINE_NAME/latest/vscode_settings.json"
                    ;;
                "generators/vscode-extensions.sh")
                    echo "  - VS Code extensions: .snapshots/$MACHINE_NAME/latest/vscode_extensions"
                    ;;
                *)
                    echo "  - $display_name: .snapshots/$MACHINE_NAME/latest/$(basename "$generator" .sh)"
                    ;;
            esac
        done
        
        echo "  - Logs: .logs/"
        echo "  - Backups: .snapshots/$MACHINE_NAME/backups/"
        
        # Run backup cleanup
        echo ""
        echo "Running backup cleanup..."
        if "$SNAPSHOT_SCRIPTS_DIR/lib/backup-manager.sh"; then
            echo "SUCCESS: Backup cleanup completed"
        else
            echo "WARNING: Backup cleanup failed (but snapshots were successful)"
        fi
    else
        echo ""
        echo "ERROR: Some snapshot operations failed"
        exit 1
    fi
}

# =============================================================================
# Main Execution
# =============================================================================

main() {
    # Parse command line arguments
    case "${1:-}" in
        --help|-h)
            show_help
            exit 0
            ;;
        --list|-l)
            list_generators
            exit 0
            ;;
        "")
            # No arguments - run all snapshots
            run_all_snapshots
            ;;
        -*)
            # Unknown option
            echo "ERROR: Unknown option: $1"
            echo ""
            show_help
            exit 1
            ;;
        *)
            # Specific generator specified - validate it exists
            if is_valid_generator "$1"; then
                run_snapshot_generator "$1" "false"
            else
                echo "ERROR: Unknown snapshot generator: $1"
                echo ""
                list_generators
                exit 1
            fi
            ;;
    esac
}

# =============================================================================
# Script Execution
# =============================================================================

# Only run main if script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi