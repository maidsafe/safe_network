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

build-release-artifacts arch nightly="false":
  #!/usr/bin/env bash
  set -e

  arch="{{arch}}"
  nightly="{{nightly}}"
  supported_archs=(
    "x86_64-pc-windows-msvc"
    "x86_64-apple-darwin"
    "aarch64-apple-darwin"
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
      sudo apt-get install -y musl-tools
    fi
  fi

  rustup target add {{arch}}

  rm -rf artifacts
  mkdir artifacts
  cargo clean

  echo "================"
  echo "= Network Keys ="
  echo "================"
  echo "FOUNDATION_PK: $FOUNDATION_PK"
  echo "GENESIS_PK: $GENESIS_PK"
  echo "NETWORK_ROYALTIES_PK: $NETWORK_ROYALTIES_PK"
  echo "PAYMENT_FORWARD_PK: $PAYMENT_FORWARD_PK"

  cross_container_opts="--env \"GENESIS_PK=$GENESIS_PK\" --env \"GENESIS_SK=$GENESIS_SK\" --env \"FOUNDATION_PK=$FOUNDATION_PK\" --env \"NETWORK_ROYALTIES_PK=$NETWORK_ROYALTIES_PK\" --env \"PAYMENT_FORWARD_PK=$PAYMENT_FORWARD_PK\""
  export CROSS_CONTAINER_OPTS=$cross_container_opts

  nightly_feature=""
  if [[ "$nightly" == "true" ]]; then
    nightly_feature="--features nightly"
  fi

  if [[ $arch == arm* || $arch == armv7* || $arch == aarch64* ]]; then
    echo "Passing to cross CROSS_CONTAINER_OPTS=$CROSS_CONTAINER_OPTS"
    cargo binstall --no-confirm cross
    cross build --release --target $arch --bin faucet --features=distribution $nightly_feature
    cross build --release --target $arch --bin nat-detection $nightly_feature
    cross build --release --target $arch --bin node-launchpad $nightly_feature
    cross build --release --features="network-contacts,distribution" --target $arch --bin safe $nightly_feature
    cross build --release --features=network-contacts --target $arch --bin safenode $nightly_feature
    cross build --release --target $arch --bin safenode-manager $nightly_feature
    cross build --release --target $arch --bin safenodemand $nightly_feature
    cross build --release --target $arch --bin safenode_rpc_client $nightly_feature
    cross build --release --target $arch --bin sn_auditor $nightly_feature
  else
    cargo build --release --target $arch --bin faucet --features=distribution $nightly_feature
    cargo build --release --target $arch --bin nat-detection $nightly_feature
    cargo build --release --target $arch --bin node-launchpad $nightly_feature
    cargo build --release --features="network-contacts,distribution" --target $arch --bin safe $nightly_feature
    cargo build --release --features=network-contacts --target $arch --bin safenode $nightly_feature
    cargo build --release --target $arch --bin safenode-manager $nightly_feature
    cargo build --release --target $arch --bin safenodemand $nightly_feature
    cargo build --release --target $arch --bin safenode_rpc_client $nightly_feature
    cargo build --release --target $arch --bin sn_auditor $nightly_feature
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
    "aarch64-apple-darwin"
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

package-all-bins:
  #!/usr/bin/env bash
  set -e
  just package-bin "faucet"
  just package-bin "nat-detection"
  just package-bin "node-launchpad"
  just package-bin "safe"
  just package-bin "safenode"
  just package-bin "safenode_rpc_client"
  just package-bin "safenode-manager"
  just package-bin "safenodemand"
  just package-bin "sn_auditor"

package-bin bin version="":
  #!/usr/bin/env bash
  set -e

  architectures=(
    "x86_64-pc-windows-msvc"
    "x86_64-apple-darwin"
    "aarch64-apple-darwin"
    "x86_64-unknown-linux-musl"
    "arm-unknown-linux-musleabi"
    "armv7-unknown-linux-musleabihf"
    "aarch64-unknown-linux-musl"
  )

  bin="{{bin}}"

  supported_bins=(\
    "faucet" \
    "nat-detection" \
    "node-launchpad" \
    "safe" \
    "safenode" \
    "safenode-manager" \
    "safenodemand" \
    "safenode_rpc_client" \
    "sn_auditor")
  crate_dir_name=""

  # In the case of the node manager, the actual name of the crate is `sn-node-manager`, but the
  # directory it's in is `sn_node_manager`.
  bin="{{bin}}"
  case "$bin" in
    faucet)
      crate_dir_name="sn_faucet"
      ;;
    nat-detection)
      crate_dir_name="nat-detection"
      ;;
    node-launchpad)
      crate_dir_name="node-launchpad"
      ;;
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
    safenode_rpc_client)
      crate_dir_name="sn_node_rpc_client"
      ;;
    sn_auditor)
      crate_dir_name="sn_auditor"
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

  rm -rf packaged_bins/$bin
  find artifacts/ -name "$bin" -exec chmod +x '{}' \;
  for arch in "${architectures[@]}" ; do
    echo "Packaging for $arch..."
    if [[ $arch == *"windows"* ]]; then bin_name="${bin}.exe"; else bin_name=$bin; fi
    zip -j $bin-$version-$arch.zip artifacts/$arch/release/$bin_name
    tar -C artifacts/$arch/release -zcvf $bin-$version-$arch.tar.gz $bin_name
  done

  mkdir -p packaged_bins/$bin
  mv *.tar.gz packaged_bins/$bin
  mv *.zip packaged_bins/$bin

upload-all-packaged-bins-to-s3:
  #!/usr/bin/env bash
  set -e

  binaries=(
    faucet
    nat-detection
    node-launchpad
    safe
    safenode
    safenode-manager
    safenode_rpc_client
    safenodemand
    sn_auditor
  )
  for binary in "${binaries[@]}"; do
    just upload-packaged-bin-to-s3 "$binary"
  done

upload-packaged-bin-to-s3 bin_name:
  #!/usr/bin/env bash
  set -e

  case "{{bin_name}}" in
    faucet)
      bucket="sn-faucet"
      ;;
    nat-detection)
      bucket="nat-detection"
      ;;
    node-launchpad)
      bucket="node-launchpad"
      ;;
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
    safenode_rpc_client)
      bucket="sn-node-rpc-client"
      ;;
    sn_auditor)
      bucket="sn-auditor"
      ;;
    *)
      echo "The {{bin_name}} binary is not supported"
      exit 1
      ;;
  esac

  cd packaged_bins/{{bin_name}}
  for file in *.zip *.tar.gz; do
    dest="s3://$bucket/$file"
    if [[ "$file" == *latest* ]]; then
      echo "Allowing overwrite for 'latest' version..."
      aws s3 cp "$file" "$dest" --acl public-read
    else
      if aws s3 ls "$dest" > /dev/null 2>&1; then
        echo "$dest already exists. Will not overwrite."
      else
        # This command outputs a lot text which makes the build log difficult to read, so we will
        # suppress it.
        aws s3 cp "$file" "$dest" --acl public-read > /dev/null 2>&1
        echo "$dest uploaded"
      fi
    fi
  done

delete-s3-bin bin_name version:
  #!/usr/bin/env bash
  set -e

  case "{{bin_name}}" in
    faucet)
      bucket="sn-faucet"
      ;;
    nat-detection)
      bucket="nat-detection"
      ;;
    node-launchpad)
      bucket="node-launchpad"
      ;;
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
    safenode_rpc_client)
      bucket="sn-node-rpc-client"
      ;;
    sn_auditor)
      bucket="sn-auditor"
      ;;
    *)
      echo "The {{bin_name}} binary is not supported"
      exit 1
      ;;
  esac

  architectures=(
    "x86_64-pc-windows-msvc"
    "x86_64-apple-darwin"
    "aarch64-apple-darwin"
    "x86_64-unknown-linux-musl"
    "arm-unknown-linux-musleabi"
    "armv7-unknown-linux-musleabihf"
    "aarch64-unknown-linux-musl"
  )

  for arch in "${architectures[@]}"; do
    zip_filename="{{bin_name}}-{{version}}-${arch}.zip"
    tar_filename="{{bin_name}}-{{version}}-${arch}.tar.gz"
    s3_zip_path="s3://$bucket/$zip_filename"
    s3_tar_path="s3://$bucket/$tar_filename"
    aws s3 rm "$s3_zip_path"
    echo "deleted $s3_zip_path"
    aws s3 rm "$s3_tar_path"
    echo "deleted $s3_tar_path"
  done

package-all-architectures:
  #!/usr/bin/env bash
  set -e

  architectures=(
    "x86_64-pc-windows-msvc"
    "x86_64-apple-darwin"
    "aarch64-apple-darwin"
    "x86_64-unknown-linux-musl"
    "arm-unknown-linux-musleabi"
    "armv7-unknown-linux-musleabihf"
    "aarch64-unknown-linux-musl"
  )

  rm -rf packaged_architectures
  for arch in "${architectures[@]}" ; do
    echo "Packaging artifacts for $arch..."
    just package-arch "$arch"
  done

package-arch arch:
  #!/usr/bin/env bash
  set -e

  if [[ -n $PACKAGE_VERSION ]]; then
    version="$PACKAGE_VERSION"
  else
    release_year=$(grep 'release-year:' release-cycle-info | awk '{print $2}')
    release_month=$(grep 'release-month:' release-cycle-info | awk '{print $2}')
    release_cycle=$(grep 'release-cycle:' release-cycle-info | awk '{print $2}')
    release_cycle_counter=$(grep 'release-cycle-counter:' release-cycle-info | awk '{print $2}')
    version="$release_year.$release_month.$release_cycle.$release_cycle_counter"
  fi
  architecture="{{arch}}"
  zip_filename="${version}.autonomi.${architecture}.zip"

  mkdir -p packaged_architectures
  cd artifacts/$architecture/release

  binaries=(
    faucet
    nat-detection
    node-launchpad
    safe
    safenode
    safenode-manager
    safenode_rpc_client
    safenodemand
    sn_auditor
  )

  if [[ "$architecture" == *"windows"* ]]; then
    for binary in "${binaries[@]}"; do
      binaries_with_extension+=("$binary.exe")
    done
    zip "../../../packaged_architectures/$zip_filename" "${binaries_with_extension[@]}"
  else
    zip "../../../packaged_architectures/$zip_filename" "${binaries[@]}"
  fi

  cd ../../..

node-man-integration-tests:
  #!/usr/bin/env bash
  set -e

  cargo build --release --bin safenode --bin faucet --bin safenode-manager
  cargo run --release --bin safenode-manager -- local run \
    --node-path target/release/safenode \
    --faucet-path target/release/faucet
  peer=$(cargo run --release --bin safenode-manager -- local status \
    --json | jq -r .nodes[-1].listen_addr[0])
  export SAFE_PEERS=$peer
  cargo test --release --package sn-node-manager --test e2e -- --nocapture
