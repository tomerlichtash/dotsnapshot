# Claude Development Rules and Configuration

## Mandatory Workflow Process

**NEVER SKIP THESE STEPS - NO EXCEPTIONS:**

1. **Create Issue First**
   - Every change must start with a GitHub issue
   - Describe the problem/feature clearly
   - Include implementation details and acceptance criteria

2. **Create Feature Branch**
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

- Always run lint and typecheck commands before committing
- Test commands: Check README or ask user for project-specific commands
- Never commit if tests are failing

## Branch Strategy

- **main:** Development and release branch
- **Feature branches:** For all development work

## Release Process

- Development happens on `main` via PRs
- Releases trigger from `main` when PR title contains **"RELEASE"**
- Only PRs with "RELEASE" in title will trigger semantic-release
- Example: `feat: Add new feature [RELEASE]`

## Repository Structure

- `docs/` - GitHub Pages documentation only
- `homebrew/` - Homebrew formula and setup (master copies)
- `src/` - Rust source code
- `.github/workflows/` - CI/CD workflows

---

**Claude: Reference this file before starting any work. These rules are non-negotiable.**