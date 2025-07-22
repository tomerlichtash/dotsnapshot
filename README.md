# Dotsnapshot

A fast, extensible Rust CLI utility that creates snapshots of dotfiles and configuration for seamless backup and restoration. The tool supports multiple plugins and uses checksums for efficient snapshot management.

## Features

- **Plugin Architecture**: Extensible plugin system for different tools and configurations
- **Plugin Hooks System**: Execute custom scripts and actions at specific points during snapshot creation
- **Async Execution**: All plugins run concurrently for better performance
- **Checksum Optimization**: Reuses existing files when content hasn't changed
- **Cross-Platform**: Works on macOS, Linux, and Windows
- **Comprehensive Logging**: Detailed logging with tracing support

## Supported Plugins

### Homebrew
- **Brewfile**: Complete package manifest for restoration

### VSCode
- **Settings**: User settings configuration (settings.json)
- **Keybindings**: Custom keybindings (keybindings.json)
- **Extensions**: Installed extensions with versions

### Cursor
- **Settings**: User settings configuration (settings.json)
- **Keybindings**: Custom keybindings (keybindings.json)
- **Extensions**: Installed extensions with versions

### NPM
- **Global Packages**: Globally installed packages and versions
- **Configuration**: NPM configuration (filtered for security)

## Installation

### From GitHub Releases (Recommended)

Download the latest release for your platform from the [releases page](https://github.com/tomerlichtash/dotsnapshot/releases):

**macOS:**
```bash
# Intel Mac
curl -L -o dotsnapshot-macos-x86_64.tar.gz https://github.com/tomerlichtash/dotsnapshot/releases/latest/download/dotsnapshot-macos-x86_64.tar.gz
tar -xzf dotsnapshot-macos-x86_64.tar.gz
sudo mv dotsnapshot /usr/local/bin/

# Apple Silicon Mac
curl -L -o dotsnapshot-macos-arm64.tar.gz https://github.com/tomerlichtash/dotsnapshot/releases/latest/download/dotsnapshot-macos-arm64.tar.gz
tar -xzf dotsnapshot-macos-arm64.tar.gz
sudo mv dotsnapshot /usr/local/bin/
```

**Linux:**
```bash
curl -L -o dotsnapshot-linux-x86_64.tar.gz https://github.com/tomerlichtash/dotsnapshot/releases/latest/download/dotsnapshot-linux-x86_64.tar.gz
tar -xzf dotsnapshot-linux-x86_64.tar.gz
sudo mv dotsnapshot /usr/local/bin/
```

**Windows:**
```bash
curl -L -o dotsnapshot-windows-x86_64.exe.zip https://github.com/tomerlichtash/dotsnapshot/releases/latest/download/dotsnapshot-windows-x86_64.exe.zip
# Extract and add to PATH
```

### Via Homebrew (macOS/Linux)

```bash
# Add tap (when available)
brew tap tomerlichtash/tools
brew install dotsnapshot
```

### From Source

```bash
git clone https://github.com/tomerlichtash/dotsnapshot.git
cd dotsnapshot
cargo build --release
sudo mv target/release/dotsnapshot /usr/local/bin/
```

### Shell Completions

After installation, you can generate shell completions:

```bash
# Bash
dotsnapshot --completions bash | sudo tee /usr/local/etc/bash_completion.d/dotsnapshot

# Zsh
dotsnapshot --completions zsh | sudo tee /usr/local/share/zsh/site-functions/_dotsnapshot

# Fish
dotsnapshot --completions fish | sudo tee /usr/local/share/fish/vendor_completions.d/dotsnapshot.fish
```

### Man Page

Generate and install the man page:

```bash
dotsnapshot --man | sudo tee /usr/local/share/man/man1/dotsnapshot.1
```

## Usage

### Basic Usage

```bash
# Create snapshot with all plugins
./target/release/dotsnapshot

# Create snapshot in custom directory
./target/release/dotsnapshot --output /path/to/snapshots

# Run specific plugins only
./target/release/dotsnapshot --plugins homebrew,npm

# Enable verbose logging
./target/release/dotsnapshot --verbose
```

### Options

- `-o, --output <PATH>`: Output directory for snapshots (overrides config file)
- `-v, --verbose`: Enable verbose logging (overrides config file)
- `-p, --plugins <PLUGINS>`: Comma-separated list of plugins to run
- `-c, --config <PATH>`: Path to config file
- `-h, --help`: Show help information

## Plugin Hooks System

Dotsnapshot supports a comprehensive hooks system that allows you to execute custom scripts and actions at specific points during the snapshot creation process.

### Hook Types

- **pre-snapshot**: Execute before any plugins run (global setup, validation)
- **post-snapshot**: Execute after all plugins complete (cleanup, notifications)
- **pre-plugin**: Execute before a specific plugin runs (plugin-specific setup)
- **post-plugin**: Execute after a specific plugin completes (plugin-specific cleanup)

### Hook Actions

#### Script Actions
Execute custom scripts with configurable arguments and timeouts:
```bash
# Add a script hook
dotsnapshot hooks add --pre-plugin homebrew_brewfile --script "brew-update.sh" --timeout 60

# Script with arguments
dotsnapshot hooks add --post-plugin homebrew_brewfile --script "validate.sh" --args "--verbose,--check"
```

#### Built-in Actions
```bash
# Logging
dotsnapshot hooks add --pre-plugin homebrew_brewfile --log "Starting Homebrew backup" --level info

# Notifications
dotsnapshot hooks add --post-snapshot --notify "Backup completed successfully" --title "dotsnapshot"

# File backup
dotsnapshot hooks add --pre-snapshot --backup --path "~/.config" --destination "/tmp/backup"

# Cleanup
dotsnapshot hooks add --post-snapshot --cleanup --temp-files --patterns "*.tmp,*.log"
```

### Hook Management Commands

#### Adding Hooks
```bash
# Plugin-specific hooks
dotsnapshot hooks add --pre-plugin homebrew_brewfile --script "setup.sh"
dotsnapshot hooks add --post-plugin vscode_settings --notify "VSCode backup complete"

# Global hooks (apply to all plugins)
dotsnapshot hooks add --pre-snapshot --script "global-setup.sh"
dotsnapshot hooks add --post-snapshot --cleanup --temp-files
```

#### Listing Hooks
```bash
# List all hooks
dotsnapshot hooks list

# List hooks for specific plugin
dotsnapshot hooks list --plugin homebrew_brewfile

# List by hook type
dotsnapshot hooks list --pre-plugin --verbose
```

#### Removing Hooks
```bash
# Remove by index
dotsnapshot hooks remove --pre-plugin homebrew_brewfile --index 0

# Remove by script name
dotsnapshot hooks remove --post-plugin homebrew_brewfile --script "setup.sh"

# Remove all hooks for a plugin
dotsnapshot hooks remove --plugin homebrew_brewfile --all
```

#### Hook Validation
```bash
# Validate all hooks
dotsnapshot hooks validate

# Validate specific plugin hooks
dotsnapshot hooks validate --plugin homebrew_brewfile
```

#### Scripts Directory Management
```bash
# Show current scripts directory
dotsnapshot hooks scripts-dir

# Set custom scripts directory
dotsnapshot hooks scripts-dir --set "~/my-scripts" --create
```

### Template Variables

Hooks support template variables that are dynamically replaced during execution:

- `{snapshot_name}` - Name of the current snapshot
- `{plugin_name}` - Name of the current plugin
- `{file_count}` - Number of files processed
- `{output_path}` - Path to the output file
- `{snapshot_dir}` - Directory of the current snapshot

Example:
```bash
dotsnapshot hooks add --post-plugin homebrew_brewfile --log "Completed {plugin_name}: {file_count} files"
```

## Configuration File

The tool supports TOML configuration files for default settings. Configuration files are searched in the following order:

1. `dotsnapshot.toml` (current directory)
2. `.dotsnapshot.toml` (current directory)
3. `~/.config/dotsnapshot/config.toml` (user config directory)
4. `~/.config/dotsnapshot.toml` (user config directory)
5. `~/.dotsnapshot.toml` (user home directory)

### Configuration Options

```toml
# Output directory for snapshots
output_dir = "/path/to/snapshots"

# Specific plugins to include (if not specified, all plugins run)
include_plugins = ["homebrew", "vscode"]

# Hooks configuration
[hooks]
scripts_dir = "~/.config/dotsnapshot/scripts"

# Global hooks (applied to all plugins)
[global.hooks]
[[global.hooks.pre-snapshot]]
action = "script"
command = "pre-snapshot-setup.sh"
timeout = 30

[[global.hooks.post-snapshot]]
action = "notify"
message = "Snapshot {snapshot_name} completed successfully"
title = "dotsnapshot"

# Plugin-specific hooks
[plugins.homebrew_brewfile.hooks]
[[plugins.homebrew_brewfile.hooks.pre-plugin]]
action = "script"
command = "brew-update.sh"
args = ["--update"]
timeout = 60

[[plugins.homebrew_brewfile.hooks.post-plugin]]
action = "log"
message = "Homebrew backup completed: {file_count} files"
level = "info"

[logging]
# Enable verbose logging by default
verbose = true

# Time format for log timestamps (uses time crate format syntax)
time_format = "[year]-[month]-[day] [hour]:[minute]:[second]"
```

### Example Configuration

```toml
# Dotsnapshot Configuration
output_dir = "/Users/username/backups/snapshots"
include_plugins = ["homebrew", "vscode", "npm"]

# Configure hooks
[hooks]
scripts_dir = "~/scripts/dotsnapshot"

# Add a simple notification when snapshots complete
[[global.hooks.post-snapshot]]
action = "notify"
message = "Backup completed at {snapshot_dir}"

# Pre-validate Homebrew before backup
[[plugins.homebrew_brewfile.hooks.pre-plugin]]
action = "script"
command = "homebrew/pre-check.sh"

[logging]
verbose = false
# Custom time format (time only)
time_format = "[hour]:[minute]:[second]"
```

**Note**: CLI arguments always override config file settings.

#### Time Format Options

The `time_format` option uses the [time crate format syntax](https://docs.rs/time/latest/time/format_description/index.html). Common format components:

- `[year]` - 4-digit year (e.g., 2025)
- `[month]` - 2-digit month (01-12)
- `[day]` - 2-digit day (01-31)
- `[hour]` - 2-digit hour (00-23)
- `[minute]` - 2-digit minute (00-59)
- `[second]` - 2-digit second (00-59)

**Examples:**
- `"[year]-[month]-[day] [hour]:[minute]:[second]"` → `2025-07-17 14:30:45` (default)
- `"[hour]:[minute]:[second]"` → `14:30:45` (time only)
- `"[month]-[day] [hour]:[minute]"` → `07-17 14:30` (short format)
- `"[year]/[month]/[day] [hour]:[minute]:[second]"` → `2025/07/17 14:30:45` (alternative date format)

## Plugin Details

### Homebrew Plugins
- **Brewfile Plugin**: Generates clean Brewfile using `brew bundle dump`
- Creates installation-ready Brewfile for `brew bundle install`
- Requires: `brew` command

### VSCode Plugins
- **Settings Plugin**: Captures user settings (settings.json)
- **Keybindings Plugin**: Captures custom keybindings (keybindings.json)
- **Extensions Plugin**: Lists installed extensions with versions
- Requires: `code` command for extensions

### Cursor Plugins
- **Settings Plugin**: Captures user settings (settings.json)
- **Keybindings Plugin**: Captures custom keybindings (keybindings.json)
- **Extensions Plugin**: Lists installed extensions with versions
- Requires: `cursor` command for extensions

### NPM Plugins
- **Global Packages Plugin**: Lists globally installed packages and versions
- **Config Plugin**: Captures NPM configuration (sensitive data filtered)
- Requires: `npm` and `node` commands

## Snapshot Structure

Each snapshot is stored in a timestamped directory:

```
snapshots/
├── 20240117_143022/
│   ├── Brewfile
│   ├── vscode_settings.json
│   ├── vscode_keybindings.json
│   ├── vscode_extensions.txt
│   ├── cursor_settings.json
│   ├── cursor_keybindings.json
│   ├── cursor_extensions.txt
│   ├── npm_global_packages.txt
│   ├── npm_config.txt
│   └── metadata.json
```

## Architecture

The project follows a clean plugin architecture with single responsibility:

- `core/`: Core functionality (plugins, checksums, snapshots, executor)
- `plugins/`: Vendor-specific plugin implementations
  - `homebrew/`: Homebrew-related plugins
  - `vscode/`: VSCode-related plugins  
  - `cursor/`: Cursor-related plugins
  - `npm/`: NPM-related plugins
- `main.rs`: CLI interface and application entry point

Each plugin focuses on a single concern and can be executed independently.

## Development

### Running Tests

```bash
cargo test
```

### Code Formatting

This project uses `rustfmt` for consistent code formatting. Run `cargo fmt --all` to format your code before committing. The CI will check formatting and fail if code is not properly formatted.

#### Optional: Pre-commit Hook

To automatically format and lint code before each commit, you can install a pre-commit hook:

```bash
./scripts/install-hooks.sh
```

This will prevent you from committing unformatted or problematic code by automatically running:
- `cargo fmt --all` - Format code according to Rust standards
- `cargo clippy` - Check for common mistakes and style issues

The hook will block commits if either check fails and provide clear guidance on fixing issues.

### Adding New Plugins

1. Create a new vendor directory in `src/plugins/` (e.g., `src/plugins/git/`)
2. Create specific plugin files for each concern (e.g., `config.rs`, `hooks.rs`)
3. Implement the `Plugin` trait for each plugin
4. Add the plugins to the registry in `main.rs`
5. Update the vendor's `mod.rs` file to export the plugins

### Plugin Requirements

Each plugin must:
- Implement the `Plugin` trait
- Focus on a single concern (settings, extensions, config, etc.)
- Be thread-safe (`Send + Sync`)
- Handle validation and error cases
- Work cross-platform where applicable
- Have a unique name and filename

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit a pull request

## License

This project is licensed under the MIT License.