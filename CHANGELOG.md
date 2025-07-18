## [1.2.1](https://github.com/tomerlichtash/dotsnapshot/compare/v1.2.0...v1.2.1) (2025-07-18)

### Bug Fixes

* Update Cargo.toml and Homebrew formula for v1.2.0 ([#15](https://github.com/tomerlichtash/dotsnapshot/issues/15)) ([1dd374b](https://github.com/tomerlichtash/dotsnapshot/commit/1dd374b7aa95feca177f8fd2489b586c431e3c58)), closes [#14](https://github.com/tomerlichtash/dotsnapshot/issues/14)

## [1.2.0](https://github.com/tomerlichtash/dotsnapshot/compare/v1.1.0...v1.2.0) (2025-07-18)

### Features

* Add Homebrew support with shell completions and man pages ([#13](https://github.com/tomerlichtash/dotsnapshot/issues/13)) ([c3d0b81](https://github.com/tomerlichtash/dotsnapshot/commit/c3d0b813357655ec4899c93df6ad6eda5bbf27b8)), closes [#12](https://github.com/tomerlichtash/dotsnapshot/issues/12)

## [1.1.0](https://github.com/tomerlichtash/dotsnapshot/compare/v1.0.0...v1.1.0) (2025-07-18)

### Features

* Add detailed info command and fix Cargo.toml issues ([#11](https://github.com/tomerlichtash/dotsnapshot/issues/11)) ([b785130](https://github.com/tomerlichtash/dotsnapshot/commit/b785130a117293a94b24a38b2a845fd19c7a8477))

## 1.0.0 (2025-07-18)

### ⚠ BREAKING CHANGES

* Releases now require semantic commit messages.
All future commits must follow conventional commit format.

🤖 Generated with [Claude Code](https://claude.ai/code)

Co-authored-by: Claude <noreply@anthropic.com>

### Features

* implement comprehensive semantic release automation ([#8](https://github.com/tomerlichtash/dotsnapshot/issues/8)) ([eb0e386](https://github.com/tomerlichtash/dotsnapshot/commit/eb0e386fcab7be8a5c7ea64dae46f8ef1d3bd5f0))
* Snapshot CLI ([#1](https://github.com/tomerlichtash/dotsnapshot/issues/1)) ([1870c62](https://github.com/tomerlichtash/dotsnapshot/commit/1870c62c4aa7cc156233c772d79a2afefd9905e3))

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
