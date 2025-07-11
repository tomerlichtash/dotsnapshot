#!/bin/bash

# =============================================================================
# DotSnapshot Installation Script
# =============================================================================
# This script installs DotSnapshot system-wide so it can be run from anywhere.
# 
# Usage:
#   ./scripts/install.sh [OPTIONS]
# 
# Options:
#   --prefix DIR     Installation prefix (default: /usr/local)
#   --user           Install for current user only
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

# Default installation settings
INSTALL_PREFIX="/usr/local"
INSTALL_FOR_USER=false
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
    echo "DotSnapshot Installation Script"
    echo "==============================="
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --prefix DIR     Installation prefix (default: /usr/local)"
    echo "  --user           Install for current user only"
    echo "  --help           Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                    # Install system-wide to /usr/local"
    echo "  $0 --prefix /opt      # Install to /opt"
    echo "  $0 --user             # Install for current user only"
    echo ""
    echo "Installation Locations:"
    echo "  System-wide: \$PREFIX/bin/dotsnapshot"
    echo "  User-only:   \$HOME/.local/bin/dotsnapshot"
    echo ""
    echo "After installation, you can run:"
    echo "  dotsnapshot --help"
    echo "  dotsnapshot --version"
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
                INSTALL_FOR_USER=true
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

setup_install_dirs() {
    if [[ "$INSTALL_FOR_USER" == "true" ]]; then
        BIN_DIR="$HOME/.local/bin"
        SHARE_DIR="$HOME/.local/share/dotsnapshot"
    else
        BIN_DIR="$INSTALL_PREFIX/bin"
        SHARE_DIR="$INSTALL_PREFIX/share/dotsnapshot"
    fi
}

check_permissions() {
    if [[ "$INSTALL_FOR_USER" == "false" ]]; then
        # Check if we can write to system directories
        if [[ ! -w "$(dirname "$BIN_DIR")" ]]; then
            log "ERROR" "Cannot write to $BIN_DIR"
            log "INFO" "Try running with sudo or use --user flag"
            exit 1
        fi
    fi
}

create_directories() {
    log "INFO" "Creating installation directories..."
    
    mkdir -p "$BIN_DIR"
    mkdir -p "$SHARE_DIR"
    
    log "SUCCESS" "Directories created"
}

install_bin_file() {
    log "INFO" "Installing dotsnapshot executable..."
    
    local bin_source="$PROJECT_ROOT/bin/dotsnapshot"
    local bin_target="$BIN_DIR/dotsnapshot"
    
    if [[ ! -f "$bin_source" ]]; then
        log "ERROR" "Bin file not found: $bin_source"
        exit 1
    fi
    
    # Copy the bin file
    cp "$bin_source" "$bin_target"
    chmod +x "$bin_target"
    
    log "SUCCESS" "Executable installed: $bin_target"
}

install_project_files() {
    log "INFO" "Installing project files..."
    
    # Copy all project files to share directory
    cp -r "$PROJECT_ROOT"/* "$SHARE_DIR/"
    
    # Remove the bin directory from share (we don't need it there)
    rm -rf "$SHARE_DIR/bin"
    
    log "SUCCESS" "Project files installed: $SHARE_DIR"
}

update_bin_file_path() {
    log "INFO" "Updating bin file to point to installation..."
    
    local bin_file="$BIN_DIR/dotsnapshot"
    
    # Create a new bin file that points to the installed location
    cat > "$bin_file" << 'EOF'
#!/bin/bash

# =============================================================================
# DotSnapshot - System-wide executable
# =============================================================================

set -euo pipefail

# Installation directory
DOTSNAPSHOT_HOME="__INSTALL_DIR__"

# Execute the main script
exec "$DOTSNAPSHOT_HOME/dotsnapshot.sh" "$@"
EOF
    
    # Replace the placeholder with actual installation directory
    sed -i.bak "s|__INSTALL_DIR__|$SHARE_DIR|g" "$bin_file"
    rm -f "$bin_file.bak"
    
    chmod +x "$bin_file"
    
    log "SUCCESS" "Bin file updated with correct path"
}

test_installation() {
    log "INFO" "Testing installation..."
    
    if command -v dotsnapshot >/dev/null 2>&1; then
        local version=$(dotsnapshot --version 2>/dev/null || echo "unknown")
        log "SUCCESS" "Installation test passed"
        log "INFO" "DotSnapshot version: $version"
    else
        log "WARNING" "dotsnapshot command not found in PATH"
        log "INFO" "You may need to add $BIN_DIR to your PATH"
    fi
}

show_post_install_info() {
    echo ""
    echo "Installation Complete!"
    echo "====================="
    echo ""
    echo "DotSnapshot has been installed to:"
    echo "  Executable: $BIN_DIR/dotsnapshot"
    echo "  Files:      $SHARE_DIR"
    echo ""
    
    if [[ "$INSTALL_FOR_USER" == "true" ]]; then
        echo "For user installation, ensure $BIN_DIR is in your PATH:"
        echo "  export PATH=\"$BIN_DIR:\$PATH\""
        echo ""
        echo "Add this to your shell profile (~/.bashrc, ~/.zshrc, etc.)"
    fi
    
    echo "Usage:"
    echo "  dotsnapshot --help"
    echo "  dotsnapshot --version"
    echo "  dotsnapshot --list"
    echo "  dotsnapshot"
    echo ""
    echo "For more information, see:"
    echo "  $SHARE_DIR/README.md"
}

# =============================================================================
# Main Execution
# =============================================================================

main() {
    log "INFO" "Starting DotSnapshot installation..."
    
    # Parse command line arguments
    parse_arguments "$@"
    
    # Setup installation directories
    setup_install_dirs
    
    log "INFO" "Installation prefix: $INSTALL_PREFIX"
    log "INFO" "Bin directory: $BIN_DIR"
    log "INFO" "Share directory: $SHARE_DIR"
    echo ""
    
    # Check permissions
    check_permissions
    
    # Create directories
    create_directories
    
    # Install files
    install_bin_file
    install_project_files
    
    # Update bin file with correct path
    update_bin_file_path
    
    # Test installation
    test_installation
    
    # Show post-install information
    show_post_install_info
}

# =============================================================================
# Script Execution
# =============================================================================

# Only run main if script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi 