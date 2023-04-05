#!/bin/bash

set -e

SAFENODE_URL_PREFIX="https://sn-node.s3.eu-west-2.amazonaws.com"

SSH_KEY_PATH=${1}
NODE_COUNT=${2:-1}
NODE_BIN_PATH=${3}
NODE_VERSION=${4}
CLIENT_COUNT=${5}
AUTO_APPROVE=${6}
OTLP_COLLECTOR_ENDPOINT=${7:-"http://dev-testnet-infra-543e2a753f964a15.elb.eu-west-2.amazonaws.com:4317"}

testnet_channel=$(terraform workspace show)
client_data_exists_file=workspace/${testnet_channel}/client-data-exists

function check_dependencies() {
  set +e
  declare -a dependecies=("terraform" "aws" "tar" "jq")
  for dependency in "${dependecies[@]}"
  do
    if ! command -v "$dependency" &> /dev/null; then
      echo "$dependency could not be found and is required"
      exit 1
    fi
  done
  set -e

  if [[ -z "${DO_PAT}" ]]; then
    echo "The DO_PAT env variable must be set with your personal access token."
    exit 1
  fi
  if [[ -z "${AWS_ACCESS_KEY_ID}" ]]; then
    echo "The AWS_ACCESS_KEY_ID env variable must be set with your access key ID."
    exit 1
  fi
  if [[ -z "${AWS_SECRET_ACCESS_KEY}" ]]; then
    echo "The AWS_SECRET_ACCESS_KEY env variable must be set with your secret access key."
    exit 1
  fi
  if [[ -z "${AWS_DEFAULT_REGION}" ]]; then
    echo "The AWS_DEFAULT_REGION env variable must be set. Default is usually eu-west-2."
    exit 1
  fi
  if [[ ! -z "${NODE_VERSION}" && ! -z "${NODE_BIN_PATH}" ]]; then
    echo "Both NODE_VERSION and NODE_BIN_PATH cannot be set at the same time."
    echo "Please use one or the other."
    exit 1
  fi
}

function run_terraform_apply() {
  local node_url="${SAFENODE_URL_PREFIX}/safenode-latest-x86_64-unknown-linux-musl.tar.gz"
  if [[ ! -z "${NODE_VERSION}" ]]; then
    node_url="${SAFENODE_URL_PREFIX}/safenode-${NODE_VERSION}-x86_64-unknown-linux-musl.tar.gz"
  elif [[ ! -z "${NODE_BIN_PATH}" ]]; then
    if [[ -d "${NODE_BIN_PATH}" ]]; then
      echo "The node bin path must be a file"
      exit 1
    fi
    local path=$(dirname "${NODE_BIN_PATH}")
  elif [[ -f "./workspace/${testnet_channel}/safenode" ]]; then
    local path=$(dirname "./workspace/${testnet_channel}/safenode")
  fi

  if [[ ! -z "${path}" ]]; then
    echo "Using node from $path"
    # The term 'custom' is used here rather than 'musl' because a locally built binary may not
    # be a musl build.
    archive_name="safenode-${testnet_channel}-x86_64-unknown-linux-custom.tar.gz"
    archive_path="/tmp/$archive_name"
    node_url="${SAFENODE_URL_PREFIX}/$archive_name"

    if test -f "$client_data_exists_file"; then
        echo "Using preexisting bin from AWS for $testnet_channel."
    else 
      echo "Creating $archive_path..."
      tar -C $path -zcvf $archive_path safenode
      echo "Uploading $archive_path to S3..."
      aws s3 cp $archive_path s3://sn-node --acl public-read
    fi
  fi

  terraform apply \
    -var "do_token=${DO_PAT}" \
    -var "pvt_key=${SSH_KEY_PATH}" \
    -var "number_of_nodes=${NODE_COUNT}" \
    -var "node_url=${node_url}" \
    -var "client_count=${CLIENT_COUNT}" \
    -var "otlp_collector_endpoint=${OTLP_COLLECTOR_ENDPOINT}" \
    --parallelism 15 ${AUTO_APPROVE}
}

function copy_ips_to_s3() {
  # This is only really used for debugging the nightly run.
  aws s3 cp \
    "./workspace/$testnet_channel/ip-list" \
    "s3://sn-node/testnet_tool/$testnet_channel-ip-list" \
    --acl public-read
  aws s3 cp \
    "./workspace/$testnet_channel/genesis-ip" \
    "s3://sn-node/testnet_tool/$testnet_channel-genesis-ip" \
    --acl public-read
}

function pull_network_contacts_and_copy_to_s3() {
  local genesis_ip=$(cat "./workspace/$testnet_channel/genesis-ip")
  local network_contacts_path="./workspace/$testnet_channel/network-contacts"
  echo "Pulling network contacts file from Genesis node"
  rsync root@"$genesis_ip":~/network-contacts "$network_contacts_path"
  aws s3 cp \
    "$network_contacts_path" \
    "s3://sn-node/testnet_tool/$testnet_channel/network-contacts" \
    --acl public-read
}

function pull_genesis_dbc_and_copy_to_s3() {
  local genesis_ip=$(cat "./workspace/$testnet_channel/genesis-ip")
  local genesis_dbc_path="./workspace/$testnet_channel/genesis-dbc"
  echo "Pulling Genesis DBC from Genesis node"
  rsync root@"$genesis_ip":~/node_data/genesis_dbc "$genesis_dbc_path"
  aws s3 cp \
    "$genesis_dbc_path" \
    "s3://sn-node/testnet_tool/$testnet_channel/genesis-dbc" \
    --acl public-read
}

function pull_genesis_key_and_copy_to_s3() {
  local genesis_ip=$(cat "./workspace/$testnet_channel/genesis-ip")
  local genesis_key_path="./workspace/$testnet_channel/genesis-key"
  echo "Pulling Genesis key from Genesis node"
  rsync root@"$genesis_ip":~/genesis-key "$genesis_key_path"
  aws s3 cp \
    "$genesis_key_path" \
    "s3://sn-node/testnet_tool/$testnet_channel-genesis-key" \
    --acl public-read
}

function kick_off_client() {
  echo "Kicking off client tests..."
  ip=$(cat workspace/${testnet_channel}/client-ip)
  echo "Safe cli version is:"
  ssh root@${ip} 'safe -V'

  if test -f "$client_data_exists_file"; then
      echo "Client data has already been put onto $testnet_channel."
  else 
    ssh root@${ip} 'safe files put loop_client_tests.sh'
    # ssh root@${ip} 'bash -ic "nohup ./loop_client_tests.sh &; bash"'
    # echo "Client tests should now be building/looping"
    ssh root@${ip} 'time safe files put -r test-data'
    echo "Test data should now exist"
    echo "data exists" > workspace/${testnet_channel}/client-data-exists
  fi

}

check_dependencies
run_terraform_apply
copy_ips_to_s3
pull_network_contacts_and_copy_to_s3
pull_genesis_dbc_and_copy_to_s3
pull_genesis_key_and_copy_to_s3
# kick_off_client
