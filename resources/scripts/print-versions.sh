#!/usr/bin/env bash

set -e

all_crates=($(awk '/members = \[/{flag=1; next} /\]/{flag=0} flag {gsub(/[",]/, ""); print $0}' \
  Cargo.toml))

echo "=================="
echo "  Crate Versions  "
echo "=================="
for crate in "${all_crates[@]}"; do
  version=$(grep "^version" < $crate/Cargo.toml | head -n 1 | awk '{ print $3 }' | sed 's/\"//g')
  echo "$crate: $version"
done

echo "==================="
echo "  Binary Versions  "
echo "==================="
echo "faucet: $(grep "^version" < sn_faucet/Cargo.toml | head -n 1 | awk '{ print $3 }' | sed 's/\"//g')"
echo "nat-detection: $(grep "^version" < nat-detection/Cargo.toml | head -n 1 | awk '{ print $3 }' | sed 's/\"//g')"
echo "node-launchpad: $(grep "^version" < node-launchpad/Cargo.toml | head -n 1 | awk '{ print $3 }' | sed 's/\"//g')"
echo "safe: $(grep "^version" < sn_cli/Cargo.toml | head -n 1 | awk '{ print $3 }' | sed 's/\"//g')"
echo "safenode: $(grep "^version" < sn_node/Cargo.toml | head -n 1 | awk '{ print $3 }' | sed 's/\"//g')"
echo "safenode-manager: $(grep "^version" < sn_node_manager/Cargo.toml | head -n 1 | awk '{ print $3 }' | sed 's/\"//g')"
echo "safenode_rpc_client: $(grep "^version" < sn_node_rpc_client/Cargo.toml | head -n 1 | awk '{ print $3 }' | sed 's/\"//g')"
echo "safenodemand: $(grep "^version" < sn_node_manager/Cargo.toml | head -n 1 | awk '{ print $3 }' | sed 's/\"//g')"
echo "sn_auditor: $(grep "^version" < sn_auditor/Cargo.toml | head -n 1 | awk '{ print $3 }' | sed 's/\"//g')"
