# Code Coverage Requirements

This project enforces a **mandatory 90% code coverage threshold** for all code changes.

## Quick Start

```bash
# Run tests with coverage check (recommended)
./scripts/test-with-coverage.sh

# Or use Makefile
make test

# Generate HTML coverage report
make coverage-html
```

## Coverage Enforcement

### Local Development
- Run `./scripts/test-with-coverage.sh` before every commit
- Script will **fail** if coverage drops below 90%
- HTML reports generated in `coverage-report/` directory

### CI/CD Pipeline
- GitHub Actions automatically enforces 90% threshold
- PRs will **fail** if coverage requirement not met
- Coverage job runs on every push and PR

## Available Commands

| Command | Description |
|---------|-------------|
| `make test` | Run tests with 90% coverage requirement |
| `make coverage` | Generate terminal coverage report |
| `make coverage-html` | Generate detailed HTML coverage report |
| `make clean` | Clean build artifacts and coverage reports |
| `make install-tools` | Install required development tools |

## Coverage Tools

- **Primary Tool**: `cargo llvm-cov` (most accurate for Rust)
- **Local Script**: `scripts/test-with-coverage.sh`
- **CI Integration**: `.github/workflows/ci.yml`
- **Configuration**: `.llvm-cov.toml`

## Current Coverage Status

As of the last cleanup: **65.27%** (needs improvement to reach 90%)

### Priority Areas for Testing

1. **Core Executor (0% coverage)** - Main execution engine
2. **Main.rs (12.37% coverage)** - CLI entry point  
3. **Command Mixins (20% coverage)** - Plugin functionality
4. **Snapshot Manager (40.46% coverage)** - Core snapshot logic

### Well-Tested Components

1. **Cursor/VSCode Extensions (95.91%)** - Excellent coverage
2. **Checksum Module (90.34%)** - Critical functionality covered
3. **Config Module (84.27%)** - Configuration handling

## Adding Tests

Focus on:
- **Integration tests** for core workflows
- **CLI command testing** with various arguments
- **Error handling** and edge cases
- **Plugin execution** in realistic scenarios

## Configuration Files

- `.llvm-cov.toml` - Coverage tool configuration
- `.gitignore` - Excludes coverage reports from git
- `CLAUDE.md` - Documents coverage requirements for AI assistant

## Bypassing Coverage (Emergency Only)

Coverage requirements should **never** be bypassed. If absolutely necessary:

1. Disable temporarily in CI by commenting out coverage job
2. Add comprehensive tests immediately after merge
3. Document the technical debt in an issue

**Remember**: High coverage ensures code quality and prevents regressions.