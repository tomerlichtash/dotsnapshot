# =============================================================================
# DotSnapshot Makefile
# =============================================================================
# This Makefile provides various development and maintenance tasks for the
# DotSnapshot project, including linting, testing, and release management.

# =============================================================================
# Configuration
# =============================================================================

# Project information
PROJECT_NAME := dotsnapshot
VERSION := $(shell cat VERSION 2>/dev/null || echo "unknown")

# Directories
SCRIPTS_DIR := scripts
LIB_DIR := lib
GENERATORS_DIR := generators
BIN_DIR := bin
CONFIG_DIR := config
TEST_DIR := test
FORMULA_DIR := Formula

# Shell scripts to lint
SCRIPTS := $(wildcard $(SCRIPTS_DIR)/*.sh)
LIB_SCRIPTS := $(wildcard $(LIB_DIR)/*.sh)
GENERATOR_SCRIPTS := $(wildcard $(GENERATORS_DIR)/*.sh)
BIN_SCRIPTS := $(wildcard $(BIN_DIR)/*)
MAIN_SCRIPT := dotsnapshot.sh

# All shell scripts
ALL_SCRIPTS := $(MAIN_SCRIPT) $(SCRIPTS) $(LIB_SCRIPTS) $(GENERATOR_SCRIPTS) $(BIN_SCRIPTS)

# ShellCheck configuration
SHELLCHECK := shellcheck
SHELLCHECK_FLAGS := --shell=bash --severity=style --color=always
SHELLCHECK_STRICT := --shell=bash --severity=warning --color=always
SHELLCHECK_ERRORS := --shell=bash --severity=error --color=always

# Colors for output
RED := \033[0;31m
GREEN := \033[0;32m
YELLOW := \033[1;33m
BLUE := \033[0;34m
NC := \033[0m # No Color

# =============================================================================
# Help
# =============================================================================

.PHONY: help
help: ## Show this help message
	@echo "$(BLUE)DotSnapshot Development Makefile$(NC)"
	@echo "=================================="
	@echo ""
	@echo "$(YELLOW)Available targets:$(NC)"
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  $(GREEN)%-20s$(NC) %s\n", $$1, $$2}' $(MAKEFILE_LIST)
	@echo ""
	@echo "$(YELLOW)Examples:$(NC)"
	@echo "  make lint          # Lint all shell scripts"
	@echo "  make lint-fix      # Lint and show fix suggestions"
	@echo "  make test          # Run all tests"
	@echo "  make install       # Install locally"
	@echo "  make clean         # Clean build artifacts"

# =============================================================================
# Linting
# =============================================================================

.PHONY: lint
lint: ## Lint all shell scripts with shellcheck
	@echo "$(BLUE)Linting shell scripts...$(NC)"
	@if ! command -v $(SHELLCHECK) >/dev/null 2>&1; then \
		echo "$(RED)Error: shellcheck not found. Install it with:$(NC)"; \
		echo "  brew install shellcheck"; \
		echo "  or visit: https://github.com/koalaman/shellcheck"; \
		exit 1; \
	fi
	@for script in $(ALL_SCRIPTS); do \
		if [ -f "$$script" ]; then \
			echo "$(BLUE)Linting: $$script$(NC)"; \
			$(SHELLCHECK) $(SHELLCHECK_FLAGS) "$$script" || exit 1; \
		fi; \
	done
	@echo "$(GREEN)✓ All scripts passed linting!$(NC)"

.PHONY: lint-fix
lint-fix: ## Lint and show fix suggestions
	@echo "$(BLUE)Linting with fix suggestions...$(NC)"
	@if ! command -v $(SHELLCHECK) >/dev/null 2>&1; then \
		echo "$(RED)Error: shellcheck not found. Install it with:$(NC)"; \
		echo "  brew install shellcheck"; \
		exit 1; \
	fi
	@for script in $(ALL_SCRIPTS); do \
		if [ -f "$$script" ]; then \
			echo "$(BLUE)Linting: $$script$(NC)"; \
			$(SHELLCHECK) $(SHELLCHECK_FLAGS) --format=diff "$$script" || true; \
		fi; \
	done

.PHONY: lint-strict
lint-strict: ## Lint with warning severity (more strict)
	@echo "$(BLUE)Linting with strict rules (warnings and errors)...$(NC)"
	@if ! command -v $(SHELLCHECK) >/dev/null 2>&1; then \
		echo "$(RED)Error: shellcheck not found. Install it with:$(NC)"; \
		echo "  brew install shellcheck"; \
		exit 1; \
	fi
	@for script in $(ALL_SCRIPTS); do \
		if [ -f "$$script" ]; then \
			echo "$(BLUE)Linting: $$script$(NC)"; \
			$(SHELLCHECK) $(SHELLCHECK_STRICT) "$$script" || exit 1; \
		fi; \
	done
	@echo "$(GREEN)✓ All scripts passed strict linting!$(NC)"

.PHONY: lint-errors
lint-errors: ## Lint with error severity only (most strict)
	@echo "$(BLUE)Linting with error severity only...$(NC)"
	@if ! command -v $(SHELLCHECK) >/dev/null 2>&1; then \
		echo "$(RED)Error: shellcheck not found. Install it with:$(NC)"; \
		echo "  brew install shellcheck"; \
		exit 1; \
	fi
	@for script in $(ALL_SCRIPTS); do \
		if [ -f "$$script" ]; then \
			echo "$(BLUE)Linting: $$script$(NC)"; \
			$(SHELLCHECK) $(SHELLCHECK_ERRORS) "$$script" || exit 1; \
		fi; \
	done
	@echo "$(GREEN)✓ All scripts passed error-only linting!$(NC)"

.PHONY: lint-json
lint-json: ## Lint and output results in JSON format
	@echo "$(BLUE)Linting and outputting JSON results...$(NC)"
	@if ! command -v $(SHELLCHECK) >/dev/null 2>&1; then \
		echo "$(RED)Error: shellcheck not found. Install it with:$(NC)"; \
		echo "  brew install shellcheck"; \
		exit 1; \
	fi
	@$(SHELLCHECK) $(SHELLCHECK_FLAGS) --format=json $(ALL_SCRIPTS) > lint-results.json 2>/dev/null || true
	@echo "$(GREEN)✓ JSON results saved to lint-results.json$(NC)"

.PHONY: lint-specific
lint-specific: ## Lint specific script (use: make lint-specific SCRIPT=path/to/script.sh)
	@if [ -z "$(SCRIPT)" ]; then \
		echo "$(RED)Error: SCRIPT not specified$(NC)"; \
		echo "Usage: make lint-specific SCRIPT=path/to/script.sh"; \
		exit 1; \
	fi
	@if [ ! -f "$(SCRIPT)" ]; then \
		echo "$(RED)Error: Script not found: $(SCRIPT)$(NC)"; \
		exit 1; \
	fi
	@echo "$(BLUE)Linting specific script: $(SCRIPT)$(NC)"
	@$(SHELLCHECK) $(SHELLCHECK_FLAGS) "$(SCRIPT)"
	@echo "$(GREEN)✓ Script passed linting!$(NC)"

.PHONY: lint-check
lint-check: ## Check if shellcheck is available
	@if command -v $(SHELLCHECK) >/dev/null 2>&1; then \
		echo "$(GREEN)✓ shellcheck is available$(NC)"; \
		$(SHELLCHECK) --version; \
	else \
		echo "$(RED)✗ shellcheck not found$(NC)"; \
		echo "Install with: brew install shellcheck"; \
		exit 1; \
	fi

# =============================================================================
# Testing
# =============================================================================

.PHONY: test
test: ## Run all tests
	@echo "$(BLUE)Running tests...$(NC)"
	@echo "$(BLUE)Testing main script...$(NC)"
	@./$(MAIN_SCRIPT) --version
	@./$(MAIN_SCRIPT) --help
	@echo "$(BLUE)Testing generators...$(NC)"
	@./$(MAIN_SCRIPT) generators/test-generator.sh
	@echo "$(BLUE)Testing backup manager...$(NC)"
	@./$(LIB_DIR)/backup-manager.sh
	@echo "$(GREEN)✓ All tests passed!$(NC)"

.PHONY: test-generators
test-generators: ## Test all generators
	@echo "$(BLUE)Testing all generators...$(NC)"
	@for generator in $(GENERATOR_SCRIPTS); do \
		if [ -f "$$generator" ]; then \
			echo "$(BLUE)Testing: $$generator$(NC)"; \
			./$(MAIN_SCRIPT) "$$generator" || exit 1; \
		fi; \
	done
	@echo "$(GREEN)✓ All generators tested!$(NC)"

# =============================================================================
# Installation
# =============================================================================

.PHONY: install
install: ## Install locally (make scripts executable)
	@echo "$(BLUE)Installing locally...$(NC)"
	@chmod +x $(MAIN_SCRIPT)
	@chmod +x $(SCRIPTS)
	@chmod +x $(LIB_SCRIPTS)
	@chmod +x $(GENERATOR_SCRIPTS)
	@chmod +x $(BIN_SCRIPTS)
	@echo "$(GREEN)✓ Installation complete!$(NC)"

.PHONY: install-system
install-system: ## Install system-wide using install script
	@echo "$(BLUE)Installing system-wide...$(NC)"
	@./$(SCRIPTS_DIR)/install.sh
	@echo "$(GREEN)✓ System installation complete!$(NC)"

# =============================================================================
# Development
# =============================================================================

.PHONY: format
format: ## Format shell scripts (basic formatting)
	@echo "$(BLUE)Formatting shell scripts...$(NC)"
	@for script in $(ALL_SCRIPTS); do \
		if [ -f "$$script" ]; then \
			echo "$(BLUE)Formatting: $$script$(NC)"; \
			# Remove trailing whitespace \
			sed -i '' 's/[[:space:]]*$$//' "$$script"; \
			# Ensure files end with newline \
			if [ -s "$$script" ] && [ "$(tail -c1 "$$script" | wc -l)" -eq 0 ]; then \
				echo "" >> "$$script"; \
			fi; \
		fi; \
	done
	@echo "$(GREEN)✓ Formatting complete!$(NC)"

.PHONY: check-syntax
check-syntax: ## Check syntax of all shell scripts
	@echo "$(BLUE)Checking shell script syntax...$(NC)"
	@for script in $(ALL_SCRIPTS); do \
		if [ -f "$$script" ]; then \
			echo "$(BLUE)Checking: $$script$(NC)"; \
			bash -n "$$script" || exit 1; \
		fi; \
	done
	@echo "$(GREEN)✓ All scripts have valid syntax!$(NC)"

# =============================================================================
# Homebrew
# =============================================================================

.PHONY: homebrew-test
homebrew-test: ## Test Homebrew formula
	@echo "$(BLUE)Testing Homebrew formula...$(NC)"
	@./$(SCRIPTS_DIR)/homebrew-setup.sh --test-formula
	@echo "$(GREEN)✓ Homebrew formula test passed!$(NC)"

.PHONY: homebrew-setup
homebrew-setup: ## Setup Homebrew formula
	@echo "$(BLUE)Setting up Homebrew formula...$(NC)"
	@./$(SCRIPTS_DIR)/homebrew-setup.sh --version $(VERSION) --github tomerlichtash
	@echo "$(GREEN)✓ Homebrew formula setup complete!$(NC)"

# =============================================================================
# Release
# =============================================================================

.PHONY: version
version: ## Show current version
	@echo "$(BLUE)Current version: $(VERSION)$(NC)"

.PHONY: bump-version
bump-version: ## Bump version (use: make bump-version TYPE=major|minor|patch)
	@if [ -z "$(TYPE)" ]; then \
		echo "$(RED)Error: TYPE not specified$(NC)"; \
		echo "Usage: make bump-version TYPE=major|minor|patch"; \
		exit 1; \
	fi
	@echo "$(BLUE)Bumping $(TYPE) version...$(NC)"
	@./$(SCRIPTS_DIR)/version.sh bump $(TYPE)
	@echo "$(GREEN)✓ Version bumped!$(NC)"

.PHONY: release
release: ## Create a release (bump version, tag, and prepare)
	@echo "$(BLUE)Creating release...$(NC)"
	@./$(SCRIPTS_DIR)/version.sh tag
	@./$(SCRIPTS_DIR)/homebrew-setup.sh --create-release
	@echo "$(GREEN)✓ Release created!$(NC)"

# =============================================================================
# Cleanup
# =============================================================================

.PHONY: clean
clean: ## Clean build artifacts and temporary files
	@echo "$(BLUE)Cleaning up...$(NC)"
	@rm -rf .snapshots
	@rm -rf .logs
	@rm -f $(SCRIPTS_DIR)/*.bak
	@rm -f $(LIB_DIR)/*.bak
	@rm -f $(FORMULA_DIR)/*.bak
	@find . -name "*.tmp" -delete
	@find . -name "*.bak" -delete
	@echo "$(GREEN)✓ Cleanup complete!$(NC)"

.PHONY: clean-all
clean-all: clean ## Clean everything including test files
	@echo "$(BLUE)Cleaning everything...$(NC)"
	@rm -rf .snapshots
	@rm -rf .logs
	@rm -rf test-*
	@echo "$(GREEN)✓ Complete cleanup done!$(NC)"

# =============================================================================
# Validation
# =============================================================================

.PHONY: validate
validate: lint check-syntax test ## Run all validation checks
	@echo "$(GREEN)✓ All validation checks passed!$(NC)"

.PHONY: pre-commit
pre-commit: format lint check-syntax ## Run pre-commit checks
	@echo "$(GREEN)✓ Pre-commit checks passed!$(NC)"

# =============================================================================
# Documentation
# =============================================================================

.PHONY: docs
docs: ## Generate documentation (placeholder)
	@echo "$(BLUE)Documentation generation not yet implemented$(NC)"
	@echo "Current documentation is in README.md and inline comments"

.PHONY: check-docs
check-docs: ## Check documentation completeness
	@echo "$(BLUE)Checking documentation...$(NC)"
	@if [ ! -f "README.md" ]; then \
		echo "$(RED)✗ README.md missing$(NC)"; \
		exit 1; \
	fi
	@if [ ! -f "CHANGELOG.md" ]; then \
		echo "$(YELLOW)⚠ CHANGELOG.md missing$(NC)"; \
	fi
	@echo "$(GREEN)✓ Documentation check complete!$(NC)"

# =============================================================================
# Default target
# =============================================================================

.DEFAULT_GOAL := help 