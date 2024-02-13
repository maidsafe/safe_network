#!/usr/bin/env bash

set -e

# optional release channel to pass in
channel_prefix=""
if [ ! -z "$1" ]; then
  channel_prefix="$1-"
fi

release-plz update 2>&1 | tee bump_version_output

crates_bumped=()
while IFS= read -r line; do
  name=$(echo "$line" | awk -F"\`" '{print $2}')
  version=$(echo "$line" | awk -F"-> " '{print $2}')
  crates_bumped+=("${channel_prefix}${name}-v${version}")
done < <(cat bump_version_output | grep "^\*")

len=${#crates_bumped[@]}
if [[ $len -eq 0 ]]; then
  echo "No changes detected. Exiting without bumping any versions."
  exit 0
fi

commit_message="chore(release): "
for crate in "${crates_bumped[@]}"; do
  commit_message="${commit_message}${crate}/"
done
commit_message=${commit_message%/} # strip off trailing '/' character

git add --all
git commit -m "$commit_message"
echo "Generated release commit: $commit_message"
