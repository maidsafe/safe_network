#!/usr/bin/env bash

# Define the workspace Cargo.toml location (ensure you're in the workspace root)
WORKSPACE_CARGO_TOML="./Cargo.toml"

# Suffix to append to the version. Passed as an argument to this script.
SUFFIX="$1"

# Ensure the suffix starts with a dash if it's provided and not empty
if [ -n "$SUFFIX" ] && [[ "$SUFFIX" != -* ]]; then
    SUFFIX="-$SUFFIX"
fi

# Check if jq is installed
if ! command -v jq > /dev/null 2>&1; then
    echo "jq is not installed. Please install jq to continue."
    exit 1
fi


# Check if the 'cargo set-version' command is available
if ! cargo set-version --help > /dev/null 2>&1; then
    echo "cargo set-version command not found."
    echo "Please install cargo-edit with the command: cargo install cargo-edit --features vendored-openssl"
    exit 1
fi

# Function to update version for a single crate with suffix
update_version_with_suffix() {
    local crate=$1
    local suffix=$2
    local current_version=$(cargo metadata --no-deps --format-version 1 | jq -r ".packages[] | select(.name == \"$crate\") | .version")
    # Perform the dry run to get the upgrade message
    local dry_run_output=$(cargo set-version -p $crate --bump patch --dry-run 2>&1)
    # Use grep and awk to extract the new version
    local new_version=$(echo "$dry_run_output" | grep "Upgrading $crate from" | awk '{print $6}')

    echo "Updating $crate from $current_version to $new_version with suffix $suffix..."
    cargo set-version -p $crate "$new_version$suffix"
}

# Function to bump patch version for the whole workspace
bump_patch_version_for_workspace() {
    echo "Bumping patch version for the whole workspace..."
    cargo set-version --bump patch
}

# Use cargo metadata and jq to parse workspace members
MEMBERS=$(cargo metadata --format-version 1 | jq -r '.workspace_members[] | split(" ") | .[0] | split("(") | .[0] | rtrimstr(")")')

if [ -n "$SUFFIX" ]; then
    # Update each crate with the new version and suffix
    for member in $MEMBERS; do
        member_name=$(echo $member | cut -d' ' -f1)
        update_version_with_suffix "$member_name" "$SUFFIX"
    done
else
    # If no suffix is provided, bump the patch version for the whole workspace
    bump_patch_version_for_workspace
fi

echo "Version update process completed."
