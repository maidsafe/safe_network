#!/usr/bin/env bash

sn_testnet_version=""
safenode_version=""

function get_crate_versions() {
  sn_testnet_version=$( \
    grep "^version" < sn_testnet/Cargo.toml | head -n 1 | awk '{ print $3 }' | sed 's/\"//g')
  safenode_version=$(grep "^version" < sn_node/Cargo.toml | head -n 1 | awk '{ print $3 }' | sed 's/\"//g')
}

function build_release_name() {
  gh_release_name="Safe Network "
  gh_release_name="${gh_release_name}v$safenode_version/"
}

function build_release_tag_name() {
  gh_release_tag_name="$sn_updater_version-"
  gh_release_tag_name="${gh_release_tag_name}$safenode_version-"
  gh_release_tag_name="${gh_release_tag_name}$sn_testnet_version"
}

gh_release_name=""
gh_release_tag_name=""
get_crate_versions
build_release_name
build_release_tag_name
