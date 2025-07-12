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

# =============================================================================
# get_snapshot_generators
# =============================================================================
# Extracts and returns all available snapshot generator script names from the
# SNAPSHOT_GENERATORS configuration array. This function parses the colon-separated
# format to extract just the script names.
#
# Parameters: None
# Returns: Space-separated list of generator script names
# Example: "generators/test-generator.sh generators/homebrew.sh generators/cursor-extensions.sh"
# =============================================================================
get_snapshot_generators() {
    local generators=()
    for generator in "${SNAPSHOT_GENERATORS[@]}"; do
        local script_name=$(echo "$generator" | cut -d':' -f1)
        generators+=("$script_name")
    done
    echo "${generators[@]}"
}

# =============================================================================
# get_display_name
# =============================================================================
# Retrieves the human-readable display name for a specific snapshot generator
# script. Searches through the SNAPSHOT_GENERATORS array to find a matching
# script name and returns its associated display name.
#
# Parameters:
#   $1 - script_name: The script name to look up (e.g., "generators/homebrew.sh")
#
# Returns: Display name on success, nothing on failure
# Exit Code: 0 on success, 1 if script not found
# Example: get_display_name "generators/homebrew.sh" returns "Brewfile"
# =============================================================================
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

# =============================================================================
# get_description
# =============================================================================
# Retrieves the description for a specific snapshot generator script. Searches
# through the SNAPSHOT_GENERATORS array to find a matching script name and
# returns its associated description.
#
# Parameters:
#   $1 - script_name: The script name to look up (e.g., "generators/homebrew.sh")
#
# Returns: Description on success, nothing on failure
# Exit Code: 0 on success, 1 if script not found
# Example: get_description "generators/homebrew.sh" returns "Creates a snapshot of Homebrew packages (Brewfile)"
# =============================================================================
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

# =============================================================================
# is_valid_generator
# =============================================================================
# Checks if a given script name corresponds to a valid snapshot generator
# that is configured in the SNAPSHOT_GENERATORS array. Used for validation
# before attempting to run a generator.
#
# Parameters:
#   $1 - script_name: The script name to validate (e.g., "generators/homebrew.sh")
#
# Returns: None
# Exit Code: 0 if valid generator, 1 if not found
# Example: is_valid_generator "generators/homebrew.sh" returns 0 (success)
# =============================================================================
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

# =============================================================================
# list_generators
# =============================================================================
# Displays a formatted list of all available snapshot generators with their
# script names, display names, and descriptions. Used for the --list command
# to show users what generators are available.
#
# Parameters: None
# Returns: None (outputs formatted list to stdout)
# Side Effects: Prints formatted generator information to console
# Example Output:
#   Available snapshot generators:
#
#     generators/test-generator.sh - Test Generator
#       Creates a test snapshot for CI testing (works on all platforms)
#
#     generators/homebrew.sh - Brewfile
#       Creates a snapshot of Homebrew packages (Brewfile)
# =============================================================================
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