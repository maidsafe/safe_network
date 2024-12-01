#!/usr/bin/env bash

set -e

if [ -z "$1" ]; then
  echo "Error: No count argument provided."
  echo "Usage: $0 <count>"
  exit 1
fi
count=$1

sudo antctl add --first --local
sudo antctl start

output=$(sudo antctl status --json)

port=$(echo "$output" | jq -r '.[0].port')
peer_id=$(echo "$output" | jq -r '.[0].peer_id')
genesis_multiaddr="/ip4/127.0.0.1/tcp/${port}/p2p/${peer_id}"

sudo antctl add --local --count "$count" --peer "$genesis_multiaddr"
sudo antctl start
antctl faucet --peer "$genesis_multiaddr"
