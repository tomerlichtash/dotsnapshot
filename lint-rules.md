# DotSnapshot Lint Rules

## Overview

This document explains the linting rules and configuration used in the DotSnapshot project.

## ShellCheck Configuration

### Configuration File
- **File**: `.shellcheckrc`
- **Purpose**: Defines custom lint rules and disabled checks
- **Location**: Project root

### Severity Levels

1. **error** - Issues that are likely to cause the script to fail
2. **warning** - Issues that may cause problems  
3. **info** - Suggestions for improvements
4. **style** - Stylistic suggestions

### Default Configuration

```bash
shell=bash
severity=style
color=always
format=tty
```

## Disabled Rules

The following ShellCheck rules are disabled in our configuration:

### SC1090 - Can't follow non-constant source
- **Reason**: We use dynamic sourcing in our scripts
- **Example**: `source "$(dirname "$SCRIPT_DIR")/lib/common.sh"`

### SC1091 - Not following source
- **Reason**: We source files that may not exist in all environments
- **Example**: Configuration files that are optional

### SC2004 - Unnecessary $ on arithmetic variables
- **Reason**: We prefer explicit $ for clarity
- **Example**: `$((count + 1))` instead of `((count + 1))`

### SC2012 - Use find instead of ls
- **Reason**: We use ls in specific cases where it's appropriate
- **Example**: Simple file listing where find is overkill

### SC2034 - Variable appears unused
- **Reason**: Some variables are used in sourced files
- **Example**: Configuration variables used by other scripts

### SC2046 - Quote to prevent word splitting
- **Reason**: We handle word splitting intentionally in some cases
- **Example**: Command substitution where splitting is desired

### SC2086 - Double quote to prevent globbing
- **Reason**: We handle this intentionally in some cases
- **Example**: Variable expansion where globbing is acceptable

### SC2115 - Use "${var:?}" to ensure expansion
- **Reason**: We handle this with our own validation
- **Example**: Custom error checking in our functions

### SC2128 - Expanding array without index
- **Reason**: We use this pattern intentionally
- **Example**: `"${array[@]}"` for all elements

### SC2148 - Add shebang
- **Reason**: We have shebangs in all files
- **Note**: All our scripts start with `#!/bin/bash`

### SC2154 - Variable referenced but not assigned
- **Reason**: Some variables are set in sourced files
- **Example**: Variables from common.sh

### SC2155 - Declare and assign separately
- **Reason**: We handle this intentionally in some cases
- **Example**: When we need to check return values

### SC2164 - Use 'cd ... || exit'
- **Reason**: We handle directory changes with our own error checking
- **Example**: Custom directory validation in our functions

### SC2181 - Check exit code directly
- **Reason**: We use $? in some cases for clarity
- **Example**: When we need to store exit codes

### SC2206 - Quote to prevent word splitting
- **Reason**: We handle this intentionally in some cases
- **Example**: Array assignments where splitting is desired

### SC2207 - Use mapfile or read -a
- **Reason**: We handle this intentionally in some cases
- **Example**: Simple command output parsing

### SC2230 - Use 'command -v' instead of which
- **Reason**: We use which in some cases for compatibility
- **Example**: Cross-platform dependency checking

### SC2231 - Quote expansions in redirection
- **Reason**: We handle this intentionally in some cases
- **Example**: When we want word splitting in redirections

## Makefile Linting Targets

### Basic Linting
```bash
make lint              # Standard linting (style level)
make lint-strict       # Strict linting (warning level)
make lint-errors       # Error-only linting (error level)
```

### Advanced Linting
```bash
make lint-fix          # Show fix suggestions
make lint-json         # Output results in JSON format
make lint-specific SCRIPT=path/to/script.sh  # Lint specific file
```

### Validation
```bash
make validate          # Run all validation checks
make pre-commit        # Run pre-commit checks
```

## Customizing Lint Rules

### Adding New Disabled Rules

To disable additional rules, add them to `.shellcheckrc`:

```bash
# Add to .shellcheckrc
disable=SC1234
disable=SC5678
```

### Changing Severity Level

Modify the severity in `.shellcheckrc`:

```bash
# For stricter linting
severity=warning

# For more lenient linting  
severity=info
```

### Project-Specific Rules

For project-specific rules, add comments in your scripts:

```bash
# shellcheck disable=SC2034
unused_variable="this is intentionally unused"

# shellcheck disable=SC2086
command $variable  # We want word splitting here
```

## Best Practices

1. **Use the configuration file** for project-wide rules
2. **Use inline comments** for script-specific exceptions
3. **Run linting regularly** as part of your development workflow
4. **Review disabled rules** periodically to ensure they're still needed
5. **Document rule exceptions** when they're not obvious

## Integration with CI/CD

The linting can be integrated into CI/CD pipelines:

```yaml
# Example GitHub Actions step
- name: Lint Shell Scripts
  run: make lint-strict
```

```bash
# Example pre-commit hook
#!/bin/bash
make pre-commit
```

## Troubleshooting

### Common Issues

1. **Rule SC1090/SC1091**: Add source files to `external-sources` in `.shellcheckrc`
2. **False positives**: Use inline comments to disable specific rules
3. **Performance**: Use `make lint-specific` for individual files during development

### Getting Help

- [ShellCheck Wiki](https://github.com/koalaman/shellcheck/wiki)
- [Rule Reference](https://github.com/koalaman/shellcheck/wiki/Checks)
- [Configuration Options](https://github.com/koalaman/shellcheck/wiki/Configuration) 