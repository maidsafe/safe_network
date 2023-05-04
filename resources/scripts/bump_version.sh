#!/usr/bin/env bash

dry_run_output=""
commit_message=""
sn_testnet=""
safenode_version=""
sn_testnet_has_changes="false"
safenode_has_changes="false"

function perform_smart_release_dry_run() {
  echo "Performing dry run for smart-release..."
  dry_run_output=$(cargo smart-release \
    --update-crates-index \
    --no-push \
    --no-publish \
    --no-changelog-preview \
    --allow-fully-generated-changelogs \
    --no-changelog-github-release \
    "sn_testnet" \
    "safenode" 2>&1)
  echo "Dry run output for smart-release:"
  echo $dry_run_output
}

function crate_has_changes() {
  local crate_name="$1"
  if [[ $dry_run_output == *"WOULD auto-bump provided package '$crate_name'"* ]] || \
     [[ $dry_run_output == *"WOULD auto-bump dependent package '$crate_name'"* ]]; then
    echo "true"
  else
    echo "false"
  fi
}

function determine_which_crates_have_changes() {
  local has_changes
  has_changes=$(crate_has_changes "sn_testnet")
  if [[ $has_changes == "true" ]]; then
    echo "smart-release has determined sn_testnet crate has changes"
    sn_testnet_has_changes="true"
  fi

  has_changes=$(crate_has_changes "safenode")
  if [[ $has_changes == "true" ]]; then
    echo "smart-release has determined safenode crate has changes"
    safenode_has_changes="true"
  fi

  if [[ $sn_testnet_has_changes == "false" ]] && \
     [[ $safenode_has_changes == "false" ]]; then
       echo "smart-release detected no changes in any crates. Exiting."
       exit 0
  fi
}

function generate_version_bump_commit() {
  echo "Running smart-release with --execute flag..."
  cargo smart-release \
    --update-crates-index \
    --no-push \
    --no-publish \
    --no-changelog-preview \
    --allow-fully-generated-changelogs \
    --no-changelog-github-release \
    --execute \
    "sn_testnet" \
     "safenode"
  exit_code=$?
  if [[ $exit_code -ne 0 ]]; then
    echo "smart-release did not run successfully. Exiting with failure code."
    exit 1
  fi
}

function generate_new_commit_message() {
  sn_testnet_version=$( \
    grep "^version" < sn_testnet/Cargo.toml | head -n 1 | awk '{ print $3 }' | sed 's/\"//g')
  safenode_version=$(grep "^version" < safenode/Cargo.toml | head -n 1 | awk '{ print $3 }' | sed 's/\"//g')
  commit_message="chore(release): "

  if [[ $sn_testnet_has_changes == "true" ]]; then
    commit_message="${commit_message}sn_testnet-${sn_testnet_version}/"
  fi
  if [[ $safenode_has_changes == "true" ]]; then
    commit_message="${commit_message}safenode-${safenode_version}/"
  fi
  commit_message=${commit_message::-1} # strip off any trailing '/'
  echo "generated commit message -- $commit_message"
}

function amend_version_bump_commit() {
  git reset --soft HEAD~1
  git add --all
  git commit -m "$commit_message"
}

function amend_tags() {
  if [[ $sn_testnet_has_changes == "true" ]]; then
    git tag "sn_testnet-v${sn_testnet_version}" -f
  fi
  if [[ $safenode_has_changes == "true" ]]; then git tag "safenode-v${safenode_version}" -f; fi
}

perform_smart_release_dry_run
determine_which_crates_have_changes
generate_version_bump_commit
generate_new_commit_message
amend_version_bump_commit
amend_tags
