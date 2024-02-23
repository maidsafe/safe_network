#!/usr/bin/env bash

set -e


# Suffix to append to the version. Passed as an argument to this script.
SUFFIX="$1"

# if there's _any_ suffix, ensure cargo set-version is installed
if [ -n "$SUFFIX" ]; then
    # Check if the 'cargo set-version' command is available
  if ! cargo set-version --help > /dev/null 2>&1; then
      echo "cargo set-version command not found."
      echo "Please install cargo-edit with the command: cargo install cargo-edit --features vendored-openssl"
      exit 1
  fi
fi

# Ensure the suffix is either alpha or beta
if [ -n "$SUFFIX" ]; then
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
  echo "No changes detected. Exiting without bumping any versions."
  exit 0
fi

# remove any performed changes if we're applying a suffix
if [ -n "$SUFFIX" ]; then
    git checkout -- .
fi

commit_message="chore(release): "
for crate in "${crates_bumped[@]}"; do
    # split the crate name and version
    crate_name=$(echo "$crate" | cut -d'v' -f1)
    # remove trailing hyphen
    crate_name=${crate_name%-}

    echo "the crate is: $crate_name"
    version=$(echo "$crate" | cut -d'v' -f2)

    # if we're changing the release channel...
    if [ -n "$SUFFIX" ]; then
        #if we're already in a realse channel, reapplying the suffix will reset things.
        if [[ "$version" == *"-alpha."* || "$version" == *"-beta."* ]]; then
          #remove any existing channel + version
            base_version=$(echo "$version" | sed -E 's/(-alpha\.[0-9]+|-beta\.[0-9]+)$//')
            new_version="${base_version}-${SUFFIX}.0"
        else
            new_version="${version}-${SUFFIX}.0"
        fi

        # set the version
        crate=$new_version
        # echo "new v for $crate_name: $new_version"
        cargo set-version -p $crate_name $new_version
    # update the commit msg
    commit_message="${commit_message}${crate_name}-v$new_version/"
    fi
done
commit_message=${commit_message%/} # strip off trailing '/' character

git add --all
git commit -m "$commit_message"
echo "Generated release commit: $commit_message"
