# Contributing to dotsnapshot

Thank you for your interest in contributing to dotsnapshot! This guide will help you get started.

## Commit Message Format

This project enforces [Conventional Commits](https://www.conventionalcommits.org/) for all commit messages. This helps us maintain a clear and consistent project history.

### Format

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

### Types

- `feat`: A new feature
- `fix`: A bug fix
- `docs`: Documentation only changes
- `style`: Changes that do not affect the meaning of the code (white-space, formatting, missing semi-colons, etc)
- `refactor`: A code change that neither fixes a bug nor adds a feature
- `perf`: A code change that improves performance
- `test`: Adding missing tests or correcting existing tests
- `build`: Changes that affect the build system or external dependencies
- `ci`: Changes to our CI configuration files and scripts
- `chore`: Other changes that don't modify src or test files
- `revert`: Reverts a previous commit

### Examples

```bash
feat: add support for zsh configuration snapshots
fix: resolve issue with VSCode settings detection
docs: update README with new plugin examples
ci: add security audit to GitHub Actions workflow
refactor: simplify plugin registry implementation
```

### Rules

- Use lowercase for the type
- Keep the header under 72 characters
- Use sentence case for the description
- Don't end the subject line with a period
- Include a blank line before the body (if present)
- Include a blank line before the footer (if present)

## Development Setup

1. Install Rust (MSRV: 1.81+)
2. Clone the repository
3. Run `cargo test` to ensure everything works
4. Make your changes
5. Run `cargo fmt` and `cargo clippy` before committing
6. Ensure all tests pass with `cargo test`

## Pull Request Process

1. Fork the repository
2. Create a feature branch from `main`
3. Make your changes following the coding standards
4. Ensure all tests pass and clippy is happy
5. Write or update tests as needed
6. Follow the commit message format above
7. Submit a pull request

All pull requests are automatically checked for:
- Code formatting (`cargo fmt`)
- Linting (`cargo clippy`)
- Tests (`cargo test`)
- Security vulnerabilities (`cargo audit`)
- Commit message format (conventional commits)

## Semantic Versioning and Releases

This project uses [Semantic Versioning](https://semver.org/) with automated releases based on commit messages:

### Version Bumping
- **Major version** (1.0.0 → 2.0.0): `feat!:` or `fix!:` with breaking changes
- **Minor version** (1.0.0 → 1.1.0): `feat:` commits
- **Patch version** (1.0.0 → 1.0.1): `fix:` commits

### Release Process
- **Automatic**: Releases are created automatically when commits are merged to `main`
- **Changelog**: Generated automatically from commit messages
- **GitHub Releases**: Created with release notes and cross-platform binaries
- **crates.io**: Published automatically (when configured)

### Breaking Changes
For breaking changes, add `!` after the type and explain the breaking change:

```bash
feat!: change plugin API to support async execution

BREAKING CHANGE: Plugin trait now requires async methods.
Update your plugins to use async/await syntax.
```

## Branch Protection

The `main` branch is protected and requires:
- Pull request reviews
- All CI checks to pass
- Semantic commit format
- No direct pushes (including from administrators)

See [BRANCH_PROTECTION.md](.github/BRANCH_PROTECTION.md) for setup details.

## Questions?

Feel free to open an issue if you have any questions about contributing!