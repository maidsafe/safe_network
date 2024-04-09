#!/usr/bin/env bash

set -e

# Suffix to append to the version. Passed as an argument to this script.
SUFFIX="$1"

# Ensure cargo set-version is installed
if ! cargo set-version --help > /dev/null 2>&1; then
  echo "cargo set-version command not found."
  echo "Please install cargo-edit with the command: cargo install cargo-edit --features vendored-openssl"
  exit 1
fi

# Ensure the suffix is either alpha or beta
if [[ -n "$SUFFIX" ]]; then
  if [[ "$SUFFIX" != "alpha" ]] && [[ "$SUFFIX" != "beta" ]]; then
    echo "Invalid suffix. Suffix must be either 'alpha' or 'beta'."
    exit 1
  fi
fi

release-plz update 2>&1 | tee bump_version_output

crates_bumped=()
while IFS= read -r line; do
  name=$(echo "$line" | awk -F"\`" '{print $2}')
  version=$(echo "$line" | awk -F"-> " '{print $2}')
  crates_bumped+=("${name}-v${version}")
done < <(cat bump_version_output | grep "^\*")

len=${#crates_bumped[@]}
if [[ $len -eq 0 ]]; then
  echo "No changes detected."
  if [[ -z "$SUFFIX" ]]; then
    echo "Removing any existing suffixes and bumping versions to stable."
    for crate in $(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | .name'); do
      version=$(cargo metadata --no-deps --format-version 1 | jq -r --arg crate_name "$crate" '.packages[] | select(.name==$crate_name) | .version')
      new_version=$(echo "$version" | sed -E 's/(-alpha\.[0-9]+|-beta\.[0-9]+)$//')
      if [[ "$version" != "$new_version" ]]; then
        echo "Removing suffix from $crate, setting version to $new_version"
        cargo set-version -p $crate $new_version
        crates_bumped+=("${crate}-v${new_version}")
      fi
    done
  fi
fi

if [[ -n "$SUFFIX" ]]; then
  echo "We are releasing to the $SUFFIX channel"
  echo "Versions with $SUFFIX are not supported by release-plz"
  echo "Reverting changes by release-plz"
  git checkout -- .
fi

commit_message="chore(release): "
for crate in "${crates_bumped[@]}"; do
  # Extract the crate name and version in a cross-platform way
  crate_name=$(echo "$crate" | sed -E 's/-v.*$//')
  version=$(echo "$crate" | sed -E 's/^.*-v(.*)$/\1/')
  new_version=$version

  echo "----------------------------------------------------------"
  echo "Processing $crate_name"
  echo "----------------------------------------------------------"
  if [[ -n "$SUFFIX" ]]; then
    # if we're already in a release channel, reapplying the suffix will reset things.
    if [[ "$version" == *"-alpha."* || "$version" == *"-beta."* ]]; then
      base_version=$(echo "$version" | sed -E 's/(-alpha\.[0-9]+|-beta\.[0-9]+)$//')
      pre_release_identifier=$(echo "$version" | sed -E 's/.*-(alpha|beta)\.([0-9]+)$/\2/')
      new_version="${base_version}-${SUFFIX}.$pre_release_identifier"
    else
      new_version="${version}-${SUFFIX}.0"
    fi
  else
    # For main release, strip any alpha or beta suffix from the version
    new_version=$(echo "$version" | sed -E 's/(-alpha\.[0-9]+|-beta\.[0-9]+)$//')
  fi

  echo "Using set-version to apply $new_version to $crate_name"
  cargo set-version -p $crate_name $new_version
  commit_message="${commit_message}${crate_name}-v$new_version/" # append crate to commit message
done
commit_message=${commit_message%/} # strip off trailing '/' character

git add --all
git commit -m "$commit_message"
echo "Generated release commit: $commit_message"
