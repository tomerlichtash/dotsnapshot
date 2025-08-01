# Claude Development Rules and Configuration

## Mandatory Workflow Process

**NEVER SKIP THESE STEPS - NO EXCEPTIONS:**

1. **Create Issue First**
   - Every change must start with a GitHub issue
   - Describe the problem/feature clearly
   - Include implementation details and acceptance criteria

2. **Create Feature Branch**
   - Ensure you're on `main` branch: `git checkout main`
   - Pull latest changes: `git pull origin main`
   - Use descriptive branch names: `feature/description`, `fix/description`
   - Always branch from `main` unless otherwise specified

3. **Follow Semantic Commit Rules**
   - **feat:** New features
   - **fix:** Bug fixes  
   - **docs:** Documentation changes
   - **style:** Code style changes (formatting, etc.)
   - **refactor:** Code refactoring
   - **test:** Adding or updating tests
   - **chore:** Maintenance tasks
   - **ci:** CI/CD changes

4. **Create Pull Request with Proper Title**
   - **Format:** `type: Description with capital letter`
   - **Max 72 characters**
   - **Must start with semantic prefix**
   - **First word after colon must be capitalized**
   - **Auto-merge:** Every new PR should be set to auto-merge
   - **Examples:**
     - ✅ `feat: Add GitHub Pages documentation site`
     - ✅ `fix: Resolve Cargo.lock version conflicts`
     - ❌ `feat: add github pages documentation site` (not capitalized)
     - ❌ `Add GitHub Pages documentation site` (no semantic prefix)

5. **Link PR to Issue**
   - Use "Addresses #X", "Closes #X", or "Fixes #X" in PR description
   - Choose based on whether PR fully resolves the issue

6. **Always Use TodoWrite Tool**
   - Track progress with TodoWrite for any multi-step work
   - Include "Create issue" and "Create PR with semantic title" as todos

## Testing and Quality

**MANDATORY COVERAGE REQUIREMENTS:**
- **90% minimum code coverage** - NO EXCEPTIONS
- Use `scripts/test-with-coverage.sh` for local testing
- CI will FAIL if coverage falls below 90%
- Focus on testing core functionality and edge cases

**Testing Workflow:**
- Always run `scripts/test-with-coverage.sh` before committing
- Always run lint and typecheck commands before committing  
- Test commands: Check README or ask user for project-specific commands
- Never commit if tests are failing or coverage is below 90%
- When adding tests, always add a comment that explains the test

**Coverage Tools:**
- Primary: `cargo llvm-cov` for accurate coverage analysis
- Local script: `scripts/test-with-coverage.sh` enforces 90% threshold
- CI enforcement: Coverage job in `.github/workflows/ci.yml`
- HTML reports: Generated in `coverage-report/` directory

## Code Quality Rules

**CRITICAL: Always use inline format arguments in Rust**
- Use `{variable_name}` instead of `{}` with positional arguments
- Use `{variable_name:?}` instead of `{:?}` for debug formatting
- Use `{variable_name}` with `.display()` calls on paths
- This applies to ALL format macros: `println!`, `format!`, `warn!`, `info!`, `error!`, etc.
- Pre-commit hooks WILL fail if positional arguments are used
- See CLIPPY_NOTES.md for detailed examples

## Branch Strategy

- **main:** Development and release branch
- **Feature branches:** For all development work

## Release Process

**Semantic Release Workflow:**

Releases are fully automated using semantic-release based on conventional commit messages:

1. **Development Commits**
   - Use semantic commit prefixes: `feat:`, `fix:`, `docs:`, etc.
   - Each commit type determines version bump:
     - `feat:` → Minor version bump (1.2.0 → 1.3.0)
     - `fix:` → Patch version bump (1.2.0 → 1.2.1)
     - `BREAKING CHANGE:` → Major version bump (1.2.0 → 2.0.0)

2. **Automatic Release Trigger**
   - When PRs are merged to `main` branch
   - Semantic-release analyzes commit messages from the merged PR
   - Automatically determines next version
   - Updates Cargo.toml and Cargo.lock
   - Creates GitHub release with changelog
   - Builds and attaches binaries for all platforms

3. **Manual Release (Emergency)**
   - Use workflow_dispatch trigger in GitHub Actions
   - Go to Actions → Release → "Run workflow"
   - Forces release even without qualifying commits

**Key Features:**
- ✅ **Fully automated** - no manual version management
- ✅ **Consistent versioning** - follows semantic versioning
- ✅ **Automatic changelog** - generated from commit messages
- ✅ **Multi-platform binaries** - built and attached automatically
- ✅ **Version alignment** - Cargo.toml, git tags, and binaries stay in sync

## Repository Structure

- `docs/` - GitHub Pages documentation only
- `homebrew/` - Homebrew formula and setup (master copies)
- `src/` - Rust source code
- `.github/workflows/` - CI/CD workflows

## Writing Guidelines

### **Language and Style**
- **NEVER use the word "comprehensive"** in PR descriptions, commit messages, or documentation
- Use specific, descriptive language instead:
  - ✅ "Complete restore functionality" 
  - ✅ "Full test coverage"
  - ✅ "Detailed implementation"
  - ❌ "Comprehensive restore functionality"
  - ❌ "Comprehensive test coverage"

## Code Organization and Design Principles

- **Test-Specific Symbols and Code**
  - When something is used only in tests, it should not be part of the main code base logic
  - For example, if you use `SYMBOL_ACTION_TEST` only in tests - it should be part of the tests, not part of the code base symbols file

## Testing Methodology

- **Test File Separation**
  - Never add `cfg(test)` inline testing in main code
  - Keep all tests in external test files
  - Separate tests into different files based on concern or aspect
  - Follow the hooks test approach: tests in dedicated, external files
  - Prevent tests from overshadowing actual code logic
  - Ensure code readability by maintaining clean separation

---

**Claude: Reference this file before starting any work. These rules are non-negotiable.**