#!/bin/bash
# Install git hooks for doodoori project

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
HOOKS_DIR="$PROJECT_ROOT/.git/hooks"

echo "Installing git hooks..."

# Create hooks directory if it doesn't exist
mkdir -p "$HOOKS_DIR"

# Install pre-commit hook
cp "$SCRIPT_DIR/pre-commit" "$HOOKS_DIR/pre-commit"
chmod +x "$HOOKS_DIR/pre-commit"
echo "  Installed: pre-commit hook (gitleaks)"

# Check if gitleaks is installed
if command -v gitleaks &> /dev/null; then
    echo ""
    echo "gitleaks version: $(gitleaks version)"
else
    echo ""
    echo "WARNING: gitleaks is not installed."
    echo "Install it with: brew install gitleaks"
fi

echo ""
echo "Git hooks installed successfully!"
