#!/usr/bin/env bash

# This script must run as the root user

set -e

if [ -z "$1" ]; then
  echo "Error: No count argument provided."
  echo "Usage: $0 <count>"
  exit 1
fi
count=$1

safenode-manager add --first --local
safenode-manager start

output=$(safenode-manager status --json)

port=$(echo "$output" | jq -r '.[0].port')
peer_id=$(echo "$output" | jq -r '.[0].peer_id')
genesis_multiaddr="/ip4/127.0.0.1/tcp/${port}/p2p/${peer_id}"

safenode-manager add --local --count "$count" --peer "$genesis_multiaddr"
safenode-manager start
