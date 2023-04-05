#!/bin/bash

set -e

SSH_KEY_PATH=${1}
REPO_OWNER=${2}
COMMIT_HASH=${3}
DEFAULT_WORKING_DIR="."
DEFAULT_COMMIT_HASH="." # git checkout . does nothing
DEFAULT_REPO_OWNER="maidsafe"
WORKING_DIR="${WORKING_DIR:-$DEFAULT_WORKING_DIR}"
REPO_OWNER="${REPO_OWNER:-$DEFAULT_REPO_OWNER}"
COMMIT_HASH="${COMMIT_HASH:-$DEFAULT_COMMIT_HASH}"

testnet_channel=$(terraform workspace show)

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

    if [[ -z "${SSH_KEY_PATH}" ]]; then
        echo "SSH key argument is missing. Usage ./build.sh <path to SSH private key>"
        exit 1
    fi
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
    if [[ ! -z "${NODE_VERSION}" && ! -z "${NODE_BIN}" ]]; then
        echo "Both NODE_VERSION and NODE_BIN cannot be set at the same time."
        echo "Please use one or the other."
        exit 1
    fi
}

function run_terraform_apply() {
    terraform apply \
         -var "do_token=${DO_PAT}" \
         -var "builder_count=1" \
         -var "pvt_key=${SSH_KEY_PATH}" -auto-approve \
         -var "repo_owner=${REPO_OWNER}" \
         -var "commit_hash=${COMMIT_HASH}" \
         -target=digitalocean_droplet.node_builder
    terraform destroy \
         -var "do_token=${DO_PAT}" \
         -var "pvt_key=${SSH_KEY_PATH}" -auto-approve \
         -target=digitalocean_droplet.node_builder
}

check_dependencies
run_terraform_apply