#!/usr/bin/env just --justfile

release_repo := "maidsafe/safe_network"

droplet-testbed:
  #!/usr/bin/env bash

  DROPLET_NAME="node-manager-testbed"
  REGION="lon1"
  SIZE="s-1vcpu-1gb"
  IMAGE="ubuntu-20-04-x64"
  SSH_KEY_ID="30878672"

  droplet_ip=$(doctl compute droplet list \
    --format Name,PublicIPv4 --no-header | grep "^$DROPLET_NAME " | awk '{ print $2 }')

  if [ -z "$droplet_ip" ]; then
    droplet_id=$(doctl compute droplet create $DROPLET_NAME \
      --region $REGION \
      --size $SIZE \
      --image $IMAGE \
      --ssh-keys $SSH_KEY_ID \
      --format ID \
      --no-header \
      --wait)
    if [ -z "$droplet_id" ]; then
      echo "Failed to obtain droplet ID"
      exit 1
    fi

    echo "Droplet ID: $droplet_id"
    echo "Waiting for droplet IP address..."
    droplet_ip=$(doctl compute droplet get $droplet_id --format PublicIPv4 --no-header)
    while [ -z "$droplet_ip" ]; do
      echo "Still waiting to obtain droplet IP address..."
      sleep 5
      droplet_ip=$(doctl compute droplet get $droplet_id --format PublicIPv4 --no-header)
    done
  fi
  echo "Droplet IP address: $droplet_ip"

  nc -zw1 $droplet_ip 22
  exit_code=$?
  while [ $exit_code -ne 0 ]; do
    echo "Waiting on SSH to become available..."
    sleep 5
    nc -zw1 $droplet_ip 22
    exit_code=$?
  done

  cargo build --release --target x86_64-unknown-linux-musl
  scp -r ./target/x86_64-unknown-linux-musl/release/safenode-manager \
    root@$droplet_ip:/root/safenode-manager

kill-testbed:
  #!/usr/bin/env bash

  DROPLET_NAME="node-manager-testbed"

  droplet_id=$(doctl compute droplet list \
    --format Name,ID --no-header | grep "^$DROPLET_NAME " | awk '{ print $2 }')

  if [ -z "$droplet_ip" ]; then
    echo "Deleting droplet with ID $droplet_id"
    doctl compute droplet delete $droplet_id
  fi

build-release-artifacts arch:
  #!/usr/bin/env bash
  set -e

  arch="{{arch}}"
  supported_archs=(
    "x86_64-pc-windows-msvc"
    "x86_64-apple-darwin"
    "x86_64-unknown-linux-musl"
    "arm-unknown-linux-musleabi"
    "armv7-unknown-linux-musleabihf"
    "aarch64-unknown-linux-musl"
  )

  arch_supported=false
  for supported_arch in "${supported_archs[@]}"; do
    if [[ "$arch" == "$supported_arch" ]]; then
      arch_supported=true
      break
    fi
  done

  if [[ "$arch_supported" == "false" ]]; then
    echo "$arch is not supported."
    exit 1
  fi

  if [[ "$arch" == "x86_64-unknown-linux-musl" ]]; then
    if [[ "$(grep -E '^NAME="Ubuntu"' /etc/os-release)" ]]; then
      # This is intended for use on a fresh Github Actions agent
      sudo apt update -y
      sudo apt install -y musl-tools
    fi
    rustup target add x86_64-unknown-linux-musl
  fi

  rm -rf artifacts
  mkdir artifacts
  cargo clean

  if [[ -n "${NETWORK_VERSION_MODE+x}"]]; then
    echo "The NETWORK_VERSION_MODE variable is set to $NETWORK_VERSION_MODE"
    export CROSS_CONTAINER_OPTS="--env NETWORK_VERSION_MODE=$NETWORK_VERSION_MODE"
  fi

  if [[ $arch == arm* || $arch == armv7* || $arch == aarch64* ]]; then
    cargo install cross
    cross build --release --features="network-contacts,distribution" --target $arch --bin safe
    cross build --release --features=network-contacts --target $arch --bin safenode
    cross build --release --target $arch --bin safenode-manager
    cross build --release --target $arch --bin safenodemand
    cross build --release --target $arch --bin faucet --features=distribution
    cross build --release --target $arch --bin safenode_rpc_client
  else
    cargo build --release --features="network-contacts,distribution" --target $arch --bin safe
    cargo build --release --features=network-contacts --target $arch --bin safenode
    cargo build --release --target $arch --bin safenode-manager
    cargo build --release --target $arch --bin safenodemand
    cargo build --release --target $arch --bin faucet --features=distribution
    cargo build --release --target $arch --bin safenode_rpc_client
  fi

  find target/$arch/release -maxdepth 1 -type f -exec cp '{}' artifacts \;
  rm -f artifacts/.cargo-lock

# Debugging target that builds an `artifacts` directory to be used with packaging targets.
#
# To use, download the artifact zip files from the workflow run and put them in an `artifacts`
# directory here. Then run the target.
make-artifacts-directory:
  #!/usr/bin/env bash
  set -e

  architectures=(
    "x86_64-pc-windows-msvc"
    "x86_64-apple-darwin"
    "x86_64-unknown-linux-musl"
    "arm-unknown-linux-musleabi"
    "armv7-unknown-linux-musleabihf"
    "aarch64-unknown-linux-musl"
  )
  cd artifacts
  for arch in "${architectures[@]}" ; do
    mkdir -p $arch/release
    unzip safe_network-$arch.zip -d $arch/release
    rm safe_network-$arch.zip
  done

package-release-assets bin version="":
  #!/usr/bin/env bash
  set -e

  architectures=(
    "x86_64-pc-windows-msvc"
    "x86_64-apple-darwin"
    "x86_64-unknown-linux-musl"
    "arm-unknown-linux-musleabi"
    "armv7-unknown-linux-musleabihf"
    "aarch64-unknown-linux-musl"
  )

  bin="{{bin}}"

  supported_bins=("safe" "safenode" "safenode-manager" "safenodemand" "faucet" "safenode_rpc_client")
  crate_dir_name=""

  # In the case of the node manager, the actual name of the crate is `sn-node-manager`, but the
  # directory it's in is `sn_node_manager`.
  bin="{{bin}}"
  case "$bin" in
    safe)
      crate_dir_name="sn_cli"
      ;;
    safenode)
      crate_dir_name="sn_node"
      ;;
    safenode-manager)
      crate_dir_name="sn_node_manager"
      ;;
    safenodemand)
      crate_dir_name="sn_node_manager"
      ;;
    faucet)
      crate_dir_name="sn_faucet"
      ;;
    safenode_rpc_client)
      crate_dir_name="sn_node_rpc_client"
      ;;
    *)
      echo "The $bin binary is not supported"
      exit 1
      ;;
  esac

  if [[ -z "{{version}}" ]]; then
    version=$(grep "^version" < $crate_dir_name/Cargo.toml | \
        head -n 1 | awk '{ print $3 }' | sed 's/\"//g')
  else
    version="{{version}}"
  fi

  if [[ -z "$version" ]]; then
    echo "Error packaging $bin. The version number was not retrieved."
    exit 1
  fi

  rm -rf deploy/$bin
  find artifacts/ -name "$bin" -exec chmod +x '{}' \;
  for arch in "${architectures[@]}" ; do
    echo "Packaging for $arch..."
    if [[ $arch == *"windows"* ]]; then bin_name="${bin}.exe"; else bin_name=$bin; fi
    zip -j $bin-$version-$arch.zip artifacts/$arch/release/$bin_name
    tar -C artifacts/$arch/release -zcvf $bin-$version-$arch.tar.gz $bin_name
  done

  mkdir -p deploy/$bin
  mv *.tar.gz deploy/$bin
  mv *.zip deploy/$bin

upload-github-release-assets:
  #!/usr/bin/env bash
  set -e

  binary_crates=(
    "sn_cli"
    "sn_node"
    "sn-node-manager"
    "sn_faucet"
    "sn_node_rpc_client"
  )

  commit_msg=$(git log -1 --pretty=%B)
  commit_msg=${commit_msg#*: } # Remove 'chore(release): ' prefix

  IFS='/' read -ra crates_with_versions <<< "$commit_msg"
  declare -a crate_names
  for crate_with_version in "${crates_with_versions[@]}"; do
    crate=$(echo "$crate_with_version" | awk -F'-v' '{print $1}')
    crates+=("$crate")
  done

  for crate in "${crates[@]}"; do
    for binary_crate in "${binary_crates[@]}"; do
      if [[ "$crate" == "$binary_crate" ]]; then
        case "$crate" in
          sn_cli)
            bin_name="safe"
            bucket="sn-cli"
            ;;
          sn_node)
            bin_name="safenode"
            bucket="sn-node"
            ;;
          sn-node-manager)
            bin_name="safenode-manager"
            bucket="sn-node-manager"
            ;;
          sn_faucet)
            bin_name="faucet"
            bucket="sn-faucet"
            ;;
          sn_node_rpc_client)
            bin_name="safenode_rpc_client"
            bucket="sn-node-rpc-client"
            ;;
          *)
            echo "The $crate crate is not supported"
            exit 1
            ;;
        esac
        # The crate_with_version variable will correspond to the tag name of the release.
        # However, only binary crates have releases, so we need to skip any tags that don't
        # correspond to a binary.
        for crate_with_version in "${crates_with_versions[@]}"; do
          if [[ $crate_with_version == $crate-v* ]]; then
            (
              cd deploy/$bin_name
              if [[ "$crate" == "sn_cli" || "$crate" == "sn_node" || "$crate" == "sn-node-manager" ]]; then
                echo "Uploading $bin_name assets to $crate_with_version release..."
                ls | xargs gh release upload $crate_with_version --repo {{release_repo}}
              fi
            )
          fi
        done
      fi
    done
  done

upload-release-assets-to-s3 bin_name:
  #!/usr/bin/env bash
  set -e

  case "{{bin_name}}" in
    safe)
      bucket="sn-cli"
      ;;
    safenode)
      bucket="sn-node"
      ;;
    safenode-manager)
      bucket="sn-node-manager"
      ;;
    safenodemand)
      bucket="sn-node-manager"
      ;;
    faucet)
      bucket="sn-faucet"
      ;;
    safenode_rpc_client)
      bucket="sn-node-rpc-client"
      ;;
    *)
      echo "The {{bin_name}} binary is not supported"
      exit 1
      ;;
  esac

  cd deploy/{{bin_name}}
  for file in *.zip *.tar.gz; do
    aws s3 cp "$file" "s3://$bucket/$file" --acl public-read
  done
