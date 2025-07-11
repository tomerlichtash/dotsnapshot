# DotSnapshot

A comprehensive dotfile and system configuration snapshot tool that creates backups of your development environment settings, packages, and configurations.

## Overview

DotSnapshot helps you maintain consistent development environments across multiple machines by creating snapshots of:
- Package managers (Homebrew, npm, etc.)
- Editor configurations (VS Code, Cursor, etc.)
- System settings and preferences
- Custom scripts and configurations

## Features

- **Automated Snapshots**: Create snapshots of your entire development environment
- **Machine-Specific Organization**: Separate snapshots by machine name
- **Backup Management**: Automatic backup creation and cleanup
- **Extensible Generator System**: Easy to add new snapshot generators
- **Comprehensive Logging**: Detailed logs for troubleshooting
- **Configuration Management**: Flexible configuration system

## Project Structure

```
dotsnapshots/
├── dotsnapshot.sh              # Main orchestrator script
├── lib/                        # Library files
│   ├── common.sh               # Common utilities and functions
│   ├── config.sh               # Generator configuration utilities
│   └── backup-manager.sh       # Backup management utilities
├── generators/                 # Snapshot generators
│   ├── homebrew.sh             # Homebrew packages snapshot
│   ├── cursor-extensions.sh    # Cursor editor extensions
│   ├── cursor-settings.sh      # Cursor editor settings
│   ├── vscode-extensions.sh    # VS Code extensions
│   └── vscode-settings.sh      # VS Code settings
├── config/                     # Configuration files
│   ├── dotsnapshot.conf        # Main configuration
│   └── generators.conf         # Generator definitions
├── test/                       # Test files
│   └── test-backup-cleanup.sh  # Backup cleanup tests
├── .logs/                      # Log files (created automatically)
└── .snapshots/                 # Snapshot output (created automatically)
```

## Quick Start

1. **Clone the repository**:
   ```bash
   git clone <repository-url>
   cd dotsnapshots
   ```

2. **Run all snapshots**:
   ```bash
   ./dotsnapshot.sh
   ```

3. **Run a specific generator**:
   ```bash
   ./dotsnapshot.sh generators/homebrew.sh
   ```

4. **List available generators**:
   ```bash
   ./dotsnapshot.sh --list
   ```

## Configuration

### Main Configuration (`config/dotsnapshot.conf`)

```bash
# Snapshot target directory
DSNP_SNAPSHOT_TARGET_DIR=".snapshots"

# Backup retention period in days
DSNP_BACKUP_RETENTION_DAYS=30

# Logs directory
DSNP_LOGS_DIR=".logs"

# Whether to use machine-specific directories
DSNP_USE_MACHINE_DIRECTORIES=true
```

### Environment Variable Overrides

You can override any configuration setting using environment variables:

```bash
export DSNP_SNAPSHOT_TARGET_DIR_ENV="/custom/path"
export DSNP_BACKUP_RETENTION_DAYS_ENV=60
export DSNP_LOGS_DIR_ENV="/var/log/dotsnapshot"
export DSNP_USE_MACHINE_DIRECTORIES_ENV=false
```

## Creating a New Generator

DotSnapshot uses a modular generator system that makes it easy to add new snapshot types. Here's how to create and add a new generator:

### Step 1: Create the Generator Script

Create a new script in the `generators/` directory. Use one of the existing generators as a template:

```bash
#!/bin/bash

# =============================================================================
# [Generator Name] Snapshot Generator
# =============================================================================
# This script creates a snapshot of [description of what it snapshots].
# 
# Usage:
#   ./generators/[generator-name].sh [backup_enabled] [timestamp]
# 
# Arguments:
#   backup_enabled: true/false - whether to create backups
#   timestamp: optional - shared timestamp for this run

set -euo pipefail

# Script configuration
SCRIPT_NAME="[generator-name].sh"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Source common utilities
source "$(dirname "$SCRIPT_DIR")/lib/common.sh"

# =============================================================================
# Generator Configuration
# =============================================================================

# Generator-specific variables
GENERATOR_NAME="[generator-name]"
OUTPUT_FILE="[output-filename]"
SOURCE_PATH="[path-to-source-file-or-command]"

# =============================================================================
# Main Generator Logic
# =============================================================================

generate_snapshot() {
    local backup_enabled="$1"
    local timestamp="${2:-}"
    
    # Initialize the snapshot
    init_snapshot "$SCRIPT_NAME" "$OUTPUT_FILE" "$backup_enabled" "$timestamp"
    
    log "STEP" "Starting $GENERATOR_NAME snapshot generation..."
    
    # Check dependencies
    check_dependency "Required Tool" "tool-command"
    
    # Create snapshot
    log "INFO" "Creating snapshot from: $SOURCE_PATH"
    
    # Your snapshot logic here
    # Example for a file:
    if [[ -f "$SOURCE_PATH" ]]; then
        cp "$SOURCE_PATH" "$LATEST_DIR/$OUTPUT_FILE"
        log "SUCCESS" "Snapshot created: $LATEST_DIR/$OUTPUT_FILE"
    else
        log "ERROR" "Source file not found: $SOURCE_PATH"
        return 1
    fi
    
    # Example for a command output:
    # if command -v "your-command" &> /dev/null; then
    #     your-command > "$LATEST_DIR/$OUTPUT_FILE"
    #     log "SUCCESS" "Snapshot created: $LATEST_DIR/$OUTPUT_FILE"
    # else
    #     log "ERROR" "Command not found: your-command"
    #     return 1
    # fi
    
    # Validate the created file
    validate_file "$LATEST_DIR/$OUTPUT_FILE" "$OUTPUT_FILE"
    
    log "SUCCESS" "$GENERATOR_NAME snapshot completed successfully"
}

# =============================================================================
# Script Execution
# =============================================================================

# Only run if script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    generate_snapshot "${1:-false}" "${2:-}"
fi
```

### Step 2: Make the Script Executable

```bash
chmod +x generators/your-generator.sh
```

### Step 3: Add to Generator Configuration

Edit `config/generators.conf` and add your generator to the `SNAPSHOT_GENERATORS` array:

```bash
SNAPSHOT_GENERATORS=(
    ...
    "generators/your-generator.sh:Your Generator:Creates a snapshot of your custom configuration"
)
```

### Step 4: Test Your Generator

1. **Test individual generator**:
   ```bash
   ./dotsnapshot.sh generators/your-generator.sh
   ```

2. **Test with all generators**:
   ```bash
   ./dotsnapshot.sh
   ```

3. **Verify output**:
   ```bash
   ls -la .snapshots/$(hostname)/latest/
   ```

## Generator Examples

### File-Based Generator (like cursor-settings.sh)

```bash
#!/bin/bash
# ... header ...

GENERATOR_NAME="cursor-settings"
OUTPUT_FILE="cursor_settings.json"
SOURCE_PATH="$HOME/Library/Application Support/Cursor/User/settings.json"

generate_snapshot() {
    local backup_enabled="$1"
    local timestamp="${2:-}"
    
    init_snapshot "$SCRIPT_NAME" "$OUTPUT_FILE" "$backup_enabled" "$timestamp"
    
    log "STEP" "Starting Cursor settings snapshot..."
    
    if [[ -f "$SOURCE_PATH" ]]; then
        cp "$SOURCE_PATH" "$LATEST_DIR/$OUTPUT_FILE"
        log "SUCCESS" "Cursor settings snapshot created"
        validate_file "$LATEST_DIR/$OUTPUT_FILE" "$OUTPUT_FILE"
    else
        log "ERROR" "Cursor settings file not found: $SOURCE_PATH"
        return 1
    fi
}
```

### Command-Based Generator (like homebrew.sh)

```bash
#!/bin/bash
# ... header ...

GENERATOR_NAME="homebrew"
OUTPUT_FILE="Brewfile"
SOURCE_PATH="brew"

generate_snapshot() {
    local backup_enabled="$1"
    local timestamp="${2:-}"
    
    init_snapshot "$SCRIPT_NAME" "$OUTPUT_FILE" "$backup_enabled" "$timestamp"
    
    log "STEP" "Starting Homebrew snapshot..."
    
    check_dependency "Homebrew" "brew"
    
    if brew bundle dump --file="$LATEST_DIR/$OUTPUT_FILE" --force; then
        log "SUCCESS" "Homebrew snapshot created"
        validate_file "$LATEST_DIR/$OUTPUT_FILE" "$OUTPUT_FILE"
    else
        log "ERROR" "Failed to create Homebrew snapshot"
        return 1
    fi
}
```

### Complex Generator (like cursor-extensions.sh)

```bash
#!/bin/bash
# ... header ...

GENERATOR_NAME="cursor-extensions"
OUTPUT_FILE="cursor_extensions"

generate_snapshot() {
    local backup_enabled="$1"
    local timestamp="${2:-}"
    
    init_snapshot "$SCRIPT_NAME" "$OUTPUT_FILE" "$backup_enabled" "$timestamp"
    
    log "STEP" "Starting Cursor extensions snapshot..."
    
    # Check if Cursor is installed
    local cursor_path="$HOME/Library/Application Support/Cursor"
    if [[ ! -d "$cursor_path" ]]; then
        log "ERROR" "Cursor not found at: $cursor_path"
        return 1
    fi
    
    # Extract extensions
    local extensions_file="$cursor_path/User/extensions/extensions.json"
    if [[ -f "$extensions_file" ]]; then
        # Parse and format extensions
        jq -r '.recommendations[]' "$extensions_file" > "$LATEST_DIR/$OUTPUT_FILE" 2>/dev/null || {
            log "WARNING" "Failed to parse extensions.json, creating empty file"
            touch "$LATEST_DIR/$OUTPUT_FILE"
        }
        log "SUCCESS" "Cursor extensions snapshot created"
        validate_file "$LATEST_DIR/$OUTPUT_FILE" "$OUTPUT_FILE"
    else
        log "ERROR" "Cursor extensions file not found: $extensions_file"
        return 1
    fi
}
```

## Best Practices

### 1. Error Handling
- Always check if source files/commands exist
- Use proper error codes and logging
- Validate output files after creation

### 2. Dependencies
- Check for required tools using `check_dependency()`
- Provide clear error messages for missing dependencies

### 3. Logging
- Use appropriate log levels (INFO, SUCCESS, WARNING, ERROR, STEP)
- Include relevant file paths and timestamps
- Log both success and failure cases

### 4. File Validation
- Always validate created files using `validate_file()`
- Check file existence and size
- Handle empty files appropriately

### 5. Backup Support
- Respect the backup parameter
- Use shared timestamps when provided
- Don't create backups for individual generator runs

### 6. Configuration
- Use descriptive generator names and descriptions
- Follow the established naming conventions
- Document any special requirements

## Troubleshooting

### Common Issues

1. **Permission Denied**: Ensure generator scripts are executable
   ```bash
   chmod +x generators/your-generator.sh
   ```

2. **Source Not Found**: Check if source files/commands exist
   ```bash
   ls -la /path/to/source/file
   which your-command
   ```

3. **Configuration Issues**: Verify generator is properly registered
   ```bash
   ./dotsnapshot.sh --list
   ```

4. **Log Files**: Check logs for detailed error information
   ```bash
   tail -f .logs/dotsnapshot.log
   ```

### Debug Mode

For detailed debugging, you can run generators with verbose output:

```bash
bash -x ./dotsnapshot.sh generators/your-generator.sh
```

## Contributing

When contributing new generators:

1. Follow the established patterns and conventions
2. Include comprehensive error handling
3. Add appropriate logging
4. Test thoroughly on different systems
5. Update this documentation if needed
6. Consider backward compatibility
