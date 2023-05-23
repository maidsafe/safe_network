#!/usr/bin/env bash

if ! command -v rg &> /dev/null; then
  echo "ripgrep could not be found and is required"
  exit 1
fi

log_dir=~/.safe/node/local-test-network

nodes_count=$(ls $log_dir | wc -l)
nodes_count=3

echo
echo "Number of existing nodes: $nodes_count"

sleep 5

count=0
while (( $count != $nodes_count ))
do
    ((count++))
    target_port=$((12000 + $count))

    echo Iteration $count
    echo Restarting node on port $target_port
    rm -rf ~/.safe/node/local-test-network/safenode-$count
    cargo run --release --example safenode_rpc_client -- "127.0.0.1:$target_port" restart 1
	sleep 25
done
