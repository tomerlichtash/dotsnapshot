# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial release of dotsnapshot CLI utility
- Support for multiple configuration snapshots:
  - Homebrew Brewfile generation
  - VSCode settings, keybindings, and extensions
  - Cursor settings, keybindings, and extensions  
  - NPM global packages and configuration
- Automated GitHub Actions CI/CD pipeline
- Semantic commit enforcement
- Comprehensive testing across multiple platforms
- Security auditing and code coverage reporting

### Features
- **Plugin System**: Extensible architecture for adding new snapshot plugins
- **Checksum Optimization**: Avoid recreating identical snapshots
- **Flexible Configuration**: Support for custom output directories and plugin selection
- **Cross-Platform Support**: Works on Linux, macOS, and Windows
- **CLI Interface**: Easy-to-use command-line interface with helpful options

### Technical Details
- Built with Rust (MSRV: 1.81)
- Async/await support for concurrent plugin execution
- Structured logging with configurable verbosity
- Comprehensive error handling and user feedback
- Automated semantic versioning and releases