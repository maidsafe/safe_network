#!/bin/bash

node_url="$1"
if [[ -z "$node_url" ]]; then
  echo "A URL for the node binary must be passed to initialise the node."
  exit 1
fi

# bind_ip_address="$3"
# if [[ -z "$bind_ip_address" ]]; then
#   echo "A bind ip address must be passed to initialise the node."
#   exit 1
# fi

# node_ip_address="$4"
# if [[ "$is_genesis" == "true" && -z "$node_ip_address" ]]; then
#   echo "A node ip address must be passed to initialise the node."
#   exit 1
# fi

port="$2"
if [[ -z "$port" ]]; then
  echo "A port must be passed to initialise the node."
  exit 1
fi

# log_level="$6"
# if [[ -z "$log_level" ]]; then
#   echo "A log level must be passed to initialise the node."
#   exit 1
# fi

node_name="$3"
if [[ -z "$node_name" ]]; then
  echo "The node name must be passed to initialize the node."
  exit 1
fi

peers="$4"
if [[ -z "$peers" ]]; then
  echo "No peer supoplied, this must be the first node"
fi

echo "PEERS PASSED:::: $peers"

# otlp_collector_endpoint="$8"
# if [[ -z "$otlp_collector_endpoint" ]]; then
#   echo "The OpenTelementry Collector endpoint must be provided to export the traces."
#   exit 1
# fi

function install_heaptrack() {
  # This is the first package we attempt to install. There are issues with apt
  # when the machine is initially used. Sometimes it is still running in the
  # background, in which case there will be an error about a file being locked.
  # Other times, the heaptrack package won't be available because it seems to
  # be some kind of timing issue: if you run the install command too quickly
  # after the update command, apt will complain it can't find the package.
  sudo DEBIAN_FRONTEND=noninteractive apt update > /dev/null 2>&1
  retry_count=1
  heaptrack_installed="false"
  while [[ $retry_count -le 20 ]]; do
    echo "Attempting to install heaptrack..."
    sudo DEBIAN_FRONTEND=noninteractive apt install ripgrep heaptrack -y > /dev/null 2>&1
    local exit_code=$?
    if [[ $exit_code -eq 0 ]]; then
        echo "heaptrack installed successfully"
        heaptrack_installed="true"
        break
    fi
    echo "Failed to install heaptrack."
    echo "Attempted $retry_count times. Will retry up to 20 times. Sleeping for 10 seconds."
    ((retry_count++))
    sleep 10
    # Without running this again there are times when it will just fail on every retry.
    sudo DEBIAN_FRONTEND=noninteractive apt update > /dev/null 2>&1
  done
  if [[ "$heaptrack_installed" == "false" ]]; then
    echo "Failed to install heaptrack"
    exit 1
  fi
}

function install_node() {
  archive_name=$(awk -F '/' '{ print $4 }' <<< $node_url)
  wget ${node_url}
  tar xf $archive_name
  chmod +x safenode
  mkdir -p ~/node_data
  mkdir -p ~/.safe/node
  mkdir -p ~/logs
}


function run_node() {
  export RUST_LOG=safenode=debug
  export RUST_LOG_OTLP=safenode=debug
  # export OTLP_SERVICE_NAME="${node_name}"
  # export OTEL_EXPORTER_OTLP_ENDPOINT="${otlp_collector_endpoint}"
  # export TOKIO_CONSOLE_BIND="${bind_ip_address}:6669",
  
  if [[ -z "$peers" ]]; then
    echo "supplied peers var is $peers"

     node_cmd=$(printf '%s' \
      "heaptrack ./safenode " \
      "--peer $peers" \
      "--root-dir ~/node_data " \
      "--log-dir ~/logs " \
      "$log_level" \
    )
  else
    node_cmd=$(printf '%s' \
      "heaptrack ./safenode " \
      "--root-dir ~/node_data " \
      "--log-dir ~/logs " \
      "$log_level" \
    )
  fi
    echo "Launching node with: $node_cmd"
    nohup sh -c "$node_cmd" &
    sleep 5
  
}

install_heaptrack
install_node
setup_network_contacts
run_node
