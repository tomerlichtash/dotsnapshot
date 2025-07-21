#!/bin/bash

# VSCode post-backup validation script
echo "💻 Validating VSCode backup..."
echo "Plugin: ${PLUGIN_NAME:-unknown}"
echo "Timestamp: $(date)"
echo "File Count: ${FILE_COUNT:-0}"

# Simulate validation
echo "📁 Checking VSCode directories..."
if [ -d "$HOME/Library/Application Support/Code" ] || [ -d "$HOME/.vscode" ] || [ -d "$HOME/.config/Code" ]; then
    echo "✅ VSCode configuration directories found"
else
    echo "⚠️  VSCode directories not found (this is fine for testing)"
fi

echo "vscode-post-backup completed at $(date)" > /tmp/dotsnapshot-vscode-post.log
echo "✅ VSCode post-backup validation completed"
exit 0