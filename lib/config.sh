#!/bin/bash

# =============================================================================
# Snapshot Generators Configuration
# =============================================================================
# This file provides utility functions for working with snapshot generators.
# The actual generators list is defined in generators.conf

# Load generators configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Check for Homebrew configuration first
if [[ -f "/usr/local/etc/dotsnapshot/generators.conf" ]]; then
    GENERATORS_CONFIG="/usr/local/etc/dotsnapshot/generators.conf"
elif [[ -f "/opt/homebrew/etc/dotsnapshot/generators.conf" ]]; then
    GENERATORS_CONFIG="/opt/homebrew/etc/dotsnapshot/generators.conf"
elif [[ -f "$SCRIPT_DIR/../config/generators.conf" ]]; then
    GENERATORS_CONFIG="$SCRIPT_DIR/../config/generators.conf"
else
    GENERATORS_CONFIG="$SCRIPT_DIR/../config/generators.conf"
fi

if [[ -f "$GENERATORS_CONFIG" ]]; then
    source "$GENERATORS_CONFIG"
else
    echo "Error: generators.conf not found at $GENERATORS_CONFIG" >&2
    exit 1
fi

# =============================================================================
# Utility Functions
# =============================================================================

# Get all available snapshot generator names
get_snapshot_generators() {
    local generators=()
    for generator in "${SNAPSHOT_GENERATORS[@]}"; do
        local script_name=$(echo "$generator" | cut -d':' -f1)
        generators+=("$script_name")
    done
    echo "${generators[@]}"
}

# Get display name for a snapshot generator
get_display_name() {
    local script_name="$1"
    for generator in "${SNAPSHOT_GENERATORS[@]}"; do
        local current_script=$(echo "$generator" | cut -d':' -f1)
        if [[ "$current_script" == "$script_name" ]]; then
            echo "$generator" | cut -d':' -f2
            return 0
        fi
    done
    return 1
}

# Get description for a snapshot generator
get_description() {
    local script_name="$1"
    for generator in "${SNAPSHOT_GENERATORS[@]}"; do
        local current_script=$(echo "$generator" | cut -d':' -f1)
        if [[ "$current_script" == "$script_name" ]]; then
            echo "$generator" | cut -d':' -f3
            return 0
        fi
    done
    return 1
}

# Check if a snapshot generator exists
is_valid_generator() {
    local script_name="$1"
    for generator in "${SNAPSHOT_GENERATORS[@]}"; do
        local current_script=$(echo "$generator" | cut -d':' -f1)
        if [[ "$current_script" == "$script_name" ]]; then
            return 0
        fi
    done
    return 1
}

# List all available snapshot generators
list_generators() {
    echo "Available snapshot generators:"
    echo ""
    for generator in "${SNAPSHOT_GENERATORS[@]}"; do
        local script_name=$(echo "$generator" | cut -d':' -f1)
        local display_name=$(echo "$generator" | cut -d':' -f2)
        local description=$(echo "$generator" | cut -d':' -f3)
        echo "  $script_name - $display_name"
        echo "    $description"
        echo ""
    done
}