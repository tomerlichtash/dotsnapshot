# Changelog

All notable changes to DotSnapshot will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial project structure and organization
- Comprehensive generator system with modular architecture
- Support for Homebrew, Cursor, and VS Code snapshots
- Machine-specific directory organization
- Automatic backup management with retention policies
- Comprehensive logging system
- Configuration management with environment variable overrides
- MIT License

### Changed
- N/A

### Deprecated
- N/A

### Removed
- N/A

### Fixed
- N/A

### Security
- N/A

## [1.0.0] - 2025-07-11

### Added
- **Core Framework**: Complete snapshot orchestration system
- **Generator System**: Modular architecture for creating snapshots
- **Homebrew Generator**: Snapshot of Homebrew packages (Brewfile)
- **Cursor Extensions Generator**: Snapshot of Cursor editor extensions
- **Cursor Settings Generator**: Snapshot of Cursor settings.json
- **VS Code Extensions Generator**: Snapshot of VS Code extensions
- **VS Code Settings Generator**: Snapshot of VS Code settings.json
- **Backup Management**: Automatic backup creation and cleanup
- **Machine Organization**: Separate snapshots by machine name
- **Configuration System**: Flexible configuration with DSNP-prefixed variables
- **Logging System**: Comprehensive logging with color-coded output
- **Error Handling**: Robust error handling and validation
- **Documentation**: Complete README with generator creation guide
- **Project Structure**: Organized directory layout (lib/, config/, generators/, test/)

### Technical Details
- **Scripts**: All scripts are POSIX-compliant bash
- **Configuration**: Environment variable overrides supported
- **Dependencies**: Minimal external dependencies
- **Cross-platform**: Designed for macOS with extensibility for other platforms
- **License**: MIT License for maximum compatibility

---

## Versioning Strategy

### Semantic Versioning (SemVer)
This project follows [Semantic Versioning 2.0.0](https://semver.org/):

- **MAJOR** version for incompatible API changes
- **MINOR** version for backwards-compatible functionality additions
- **PATCH** version for backwards-compatible bug fixes

### Version Components
- **1.0.0**: Major.Minor.Patch
- **Pre-release**: 1.0.0-alpha.1, 1.0.0-beta.1, 1.0.0-rc.1
- **Build metadata**: 1.0.0+build.1

### Release Types
1. **Major Release** (1.0.0 → 2.0.0): Breaking changes
2. **Minor Release** (1.0.0 → 1.1.0): New features, backwards compatible
3. **Patch Release** (1.0.0 → 1.0.1): Bug fixes, backwards compatible
4. **Pre-release** (1.0.0 → 1.0.0-alpha.1): Development/testing versions

### Version Management
- **VERSION file**: Single source of truth for current version
- **Git tags**: Tagged releases for version control
- **Changelog**: Detailed change history
- **Release notes**: User-friendly release summaries