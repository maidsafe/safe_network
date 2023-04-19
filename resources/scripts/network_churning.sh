#!/usr/bin/env bash

if ! command -v rg &> /dev/null; then
  echo "ripgrep could not be found and is required"
  exit 1
fi

nodes=$(\
  rg ".*PID: .*" "$log_dir" -g "*.log*" -u | \
  rg ".*PID: (\d{3}.*\)).*" -or '$1->$2')
nodes_count=$(echo "$nodes" | wc -l)

echo
echo "Number of existing nodes: $nodes_count"

sleep 5

count=0
node_count=25
while (( $count != 10 ))
do
    ((count++))
    echo Iteration $count
    echo Kill node $nodes[$count] first
	kill -9 $nodes[$count]
	sleep 5
	((node_count++))
    echo Join a new node as $node_count
	./target/release/testnet -j -c 1
	sleep 5
done
