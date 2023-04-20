#!/usr/bin/env bash

if ! command -v rg &> /dev/null; then
  echo "ripgrep could not be found and is required"
  exit 1
fi

log_dir=~/.safe/node/local-test-network

nodes_count=$(ls $log_dir | wc -l)

echo
echo "Number of existing nodes: $nodes_count"

sleep 5

count=0
node_count=25
while (( $count != 10 ))
do
    ((count++))
    echo Iteration $count
    echo Restarting node $count
    cargo run --release --example safenode_rpc_client -- "127.0.0.1:1200$count" restart 5000
	sleep 5
done
