#!/usr/bin/env bash

# Parse members from Cargo.toml using tomlq
members=()
while IFS= read -r line; do
    members+=("$line")
done < <(tomlq -r '.workspace.members[]' Cargo.toml)

# Loop over each member and update its version
for member in "${members[@]}"
do
  # Fetch the latest version number from crates.io
  latest_version=$(curl -s "https://crates.io/api/v1/crates/$member" | jq -r '.crate.newest_version')

  # Check if we got a valid version
  if [[ "$latest_version" != "null" ]]; then
    echo "Updating $member to version $latest_version"
    # Set the version in the local Cargo.toml
    cargo set-version --package $member $latest_version
  else
    echo "Failed to fetch version for $member"
  fi
done
