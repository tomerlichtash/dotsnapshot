#!/bin/bash
#
# Install pre-commit hooks for dotsnapshot development
#
# This script installs a pre-commit hook that automatically runs cargo fmt
# and prevents commits if the code is not properly formatted.
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}Installing pre-commit hooks for dotsnapshot...${NC}"

# Check if we're in a git repository
if [ ! -d ".git" ]; then
    echo -e "${RED}Error: Not in a git repository. Please run this script from the project root.${NC}"
    exit 1
fi

# Check if cargo is available
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Error: cargo command not found. Please install Rust and Cargo.${NC}"
    exit 1
fi

# Create hooks directory if it doesn't exist
mkdir -p .git/hooks

# Create the pre-commit hook
cat > .git/hooks/pre-commit << 'EOF'
#!/bin/sh
#
# Pre-commit hook that runs cargo fmt and fails if code is not formatted
#

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "${YELLOW}Running cargo fmt...${NC}"

# Run cargo fmt
cargo fmt --all

# Check if there are any changes after formatting (check working directory changes)
if ! git diff --quiet; then
    echo "${RED}❌ Code was not properly formatted!${NC}"
    echo "${YELLOW}The following files have been formatted:${NC}"
    git diff --name-only
    echo ""
    echo "${YELLOW}Please add the formatted files and commit again:${NC}"
    echo "  git add ."
    echo "  git commit"
    exit 1
fi

echo "${GREEN}✅ Code is properly formatted${NC}"
exit 0
EOF

# Make the hook executable
chmod +x .git/hooks/pre-commit

echo -e "${GREEN}✅ Pre-commit hook installed successfully!${NC}"
echo ""
echo "The hook will:"
echo "  • Run 'cargo fmt --all' before each commit"
echo "  • Block commits if code is not properly formatted"
echo "  • Show which files were formatted and provide clear instructions"
echo ""
echo -e "${YELLOW}To uninstall the hook, simply delete:${NC} .git/hooks/pre-commit"