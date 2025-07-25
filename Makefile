# Development Makefile for dotsnapshot
# Provides convenient commands for common development tasks

.PHONY: test coverage coverage-html clean lint clippy fmt check install-tools help

# Default target
help:
	@echo "Available commands:"
	@echo "  test         - Run tests with 90% coverage requirement"
	@echo "  coverage     - Generate coverage report in terminal"
	@echo "  coverage-html- Generate HTML coverage report"
	@echo "  clean        - Clean build artifacts and coverage reports"
	@echo "  lint         - Run cargo fmt and clippy"
	@echo "  check        - Run all quality checks (fmt, clippy, test with coverage)"
	@echo "  install-tools- Install required development tools"

# Run tests with mandatory 90% coverage check
test:
	@./scripts/test-with-coverage.sh

# Generate coverage report in terminal
coverage:
	@echo "🔍 Generating coverage report..."
	@cargo llvm-cov --all-features --workspace

# Generate HTML coverage report
coverage-html:
	@echo "📄 Generating HTML coverage report..."
	@cargo llvm-cov --all-features --workspace --html --output-dir coverage-report
	@echo "🔗 Open coverage-report/index.html to view detailed report"

# Clean build artifacts and coverage reports
clean:
	@echo "🧹 Cleaning build artifacts and coverage reports..."
	@cargo clean
	@rm -rf coverage-report/ lcov.info coverage/

# Run formatting and linting
lint:
	@echo "🔧 Running code formatting..."
	@cargo fmt --all
	@echo "📋 Running clippy..."
	@cargo clippy --all-targets --all-features -- -D warnings

# Alias for lint
fmt: lint

# Run all quality checks
check: lint test
	@echo "✅ All quality checks passed!"

# Install required development tools
install-tools:
	@echo "🛠️  Installing development tools..."
	@rustup component add llvm-tools-preview
	@cargo install cargo-llvm-cov
	@echo "✅ Development tools installed!"