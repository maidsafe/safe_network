# The Safe Network

[SafenetForum.org](https://safenetforum.org/)

Own your data. Share your disk space. Get paid for doing so.<br>
The Data on the Safe Network is Decentralised, Autonomous, and built atop of Kademlia and
Libp2p.<br>

## Table of Contents

- [For Users](#for-Users)
- [For Developers](#for-developers)
- [For the Technical](#for-the-technical)
- [Using a Local Network](#Using-a-local-network)
- [Metrics Dashboard](#metrics-dashboard)

### For Users

- [CLI](https://github.com/maidsafe/safe_network/blob/main/sn_cli/README.md) The Command Line
  Interface, allowing users to interact with the network from their terminal.
- [Node](https://github.com/maidsafe//safe_network/blob/main/sn_node/README.md) The backbone of the
  safe network. Nodes can be run on commodity hardware and provide storage space and validation of
  transactions to the network.

### For Developers

- [Client](https://github.com/maidsafe/safe_network/blob/main/sn_client/README.md) The client APIs
  allowing use of the SafeNetwork to users and developers.
- [Registers](https://github.com/maidsafe/safe_network/blob/main/sn_registers/README.md) The CRDT
  registers structures available on the network.
- [Node Manager](https://github.com/maidsafe/safe_network/blob/main/sn_node_manager/README.md) Use
  to create a local network for development and testing.
- [Faucet](https://github.com/maidsafe/safe_network/blob/main/sn_faucet/README.md) The local faucet
  server, used to claim genesis and request tokens from the network.
- [Node RPC](https://github.com/maidsafe/safe_network/blob/main/sn_node_rpc_client/README.md) The
  RPC server used by the nodes to expose API calls to the outside world.

#### Releases

` ./resources/scripts/bump_version.sh` will bump the version of the crates in the `Cargo.toml`
files. And generate a `chore(release):` commit, which if pushed
to main will result in CI doing a full release run.

` ./resources/scripts/bump_version.sh` can also be namespaced for other release
channels. eg `./resources/scripts/bump_version.sh beta` will bump the version to
a `beta` release on any changed crates.

#### Transport Protocols and Architectures

The Safe Network uses `quic` as the default transport protocol.

The `websockets` feature is available for the `sn_networking` crate, and above, and will allow for
tcp over websockets.

If building for `wasm32` then `websockets` are enabled by default as this is the only method
avilable to communicate with a network as things stand. (And that network must have `websockets`
enabled.)

##### Building for wasm32

- Install [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)
- `cd sn_client && wasm-pack build`

You can then pull this package into a web app eg, to use it.

eg `await safe.get_data("/ip4/127.0.0.1/tcp/59324/ws/p2p/12D3KooWG6kyBwLVHj5hYK2SqGkP4GqrCz5gfwsvPBYic4c4TeUz","9d7e115061066126482a229822e6d68737bd67d826c269762c0f64ce87af6b4c")`

#### Browser usage

Browser usage is highly experimental, but the wasm32 target for `sn_client` _should_ work here.
YMMV until stabilised.

### For the Technical

- [Logging](https://github.com/maidsafe/safe_network/blob/main/sn_logging/README.md) The
  generalised logging crate used by the safe network (backed by the tracing crate).
- [Metrics](https://github.com/maidsafe/safe_network/blob/main/metrics/README.md) The metrics crate
  used by the safe network.
- [Networking](https://github.com/maidsafe/safe_network/blob/main/sn_networking/README.md) The
  networking layer, built atop libp2p which allows nodes and clients to communicate.
- [Protocol](https://github.com/maidsafe/safe_network/blob/main/sn_protocol/README.md) The protocol
  used by the safe network.
- [Transfers](https://github.com/maidsafe/safe_network/blob/main/sn_transfers/README.md) The
  transfers crate, used to send and receive tokens on the network.
- [Peers Acquisition](https://github.com/maidsafe/safe_network/blob/main/sn_peers_acquisition/README.md)
  The peers peers acqisition crate, or: how the network layer discovers bootstrap peers.
- [Build Info](https://github.com/maidsafe/safe_network/blob/main/sn_build_info/README.md) Small
  helper used to get the build/commit versioning info for debug purposes.

## Using a Local Network

We can explore the network's features by using multiple node processes to form a local network.

The latest version of [Rust](https://www.rust-lang.org/learn/get-started) should be installed. If
you already have an installation, use `rustup update` to get the latest version.

Run all the commands from the root of this repository.

### Run the Network

Follow these steps to create a local network:

1. Create the test network: <br>

```bash
cargo run --bin safenode-manager --features local-discovery -- local run --build
```

2. Verify node status: <br>

```bash
cargo run --bin safenode-manager --features local-discovery -- status
```

3. Build a tokenized wallet: <br>

```bash
cargo run --bin safe --features local-discovery -- wallet get-faucet 127.0.0.1:8000
```

The node manager's `run` command starts the node processes and a faucet process, the latter of
which will dispense tokens for use with the network. The `status` command should show twenty-five
running nodes. The `wallet` command retrieves some tokens, which enables file uploads.

### Files

The file storage capability can be demonstrated by uploading files to the local network, then
retrieving them.

Upload a file or a directory:

```bash
cargo run --bin safe --features local-discovery -- files upload <path>
```

The output will show that the upload costs some tokens.

Now download the files again:

```bash
cargo run --bin safe --features local-discovery -- files download
```

### Folders

The folders storage capability can be demonstrated by storing folders on the network, making
changes and syncing them with the stored version on the network, as well as downloading the entire
folders hierarchy onto a local directory.

All the following commands act on the current directory by default, but since we are building the
CLI binary to run it, we will have to always provide the directory we want them to act as a path
argument.
When otherwise running directly an already built CLI binary, we can simply make sure we are located
at the directory we want to act on without the need of providing the path as argument.

Initialise a directory to then be able to track changes made on it, and sync them up with the
network:

```bash
cargo run --bin safe --features local-discovery -- folders init <dir-path>
```

Make sure you made a backup copy of the "recovery secret" generated by the above command, or the
one you have provided when prompted.

If any changes are now made to files or directories within this folder (at this point all files and
folders are considered new since it has just been initalised for tracking), before trying to push
those changes to the network, we can get a report of the changes that have been made locally:

```bash
cargo run --bin safe --features local-discovery -- folders status <dir-path>
```

We can now push all local changes made to files and directories to the network, as well as pull any
changes that could have been made to the version stored on the network since last time we synced
with it:

```bash
cargo run --bin safe --features local-discovery -- folders sync <dir-path>
```

Now that's all stored on the network, you can download the folders onto any other path by providing
it as the target directory to the following command (you will be prompted to enter the "recovery
secret" you obtained when initialising the directory with `init` command):

```bash
cargo run --bin safe --features local-discovery -- folders download <target dir path>
```

### Token Transfers

Use your local wallet to demonstrate sending tokens and receiving transfers.

First, get your wallet address:

```
cargo run --bin safe -- wallet address
```

Now send some tokens to that address:

```
cargo run --bin safe --features local-discovery -- wallet send 2 [address]
```

This will output a transfer as a hex string, which should be sent to the recipient out-of-band.

For demonstration purposes, copy the transfer string and use it to receive the transfer in your own
wallet:

```
cargo run --bin safe --features local-discovery -- wallet receive [transfer]
```

#### Out of band transaction signing

When you want to transfer tokens from a cold storage or hardware wallet, you can create and sign
the transaction offline. This is done to prevent the private key from being exposed to any online
threats.
For this type of scenarios you can create a watch-only wallet (it holds only a public key) on the
online device, while using a hot-wallet (which holds the secret key) on a device that is offline.
The following steps are a simple guide for performing such an operation.

Steps on the online device/computer with a watch-only wallet:

1. Create a watch-only wallet using the hex-encoded public key:
   `cargo run --release --bin safe -- wowallet create <hex-encoded public key>`

2. Deposit a cash-note, owned by the public key used above when creating, into the watch-only
   wallet:
   `cargo run --release --bin safe -- wowallet deposit <hex-encoded public key> --cash-note <hex-encoded cash-note>`

3. Build an unsigned transaction:
   `cargo run --release --bin safe -- wowallet transaction <hex-encoded public key> <amount> <recipient's hex-encoded public key>`

4. Copy the built unsigned Tx generated by the above command, and send it out-of-band to the
   desired device where the hot-wallet can be loaded.

Steps on the offline device/computer with the corresponding hot-wallet:

5. If you still don't have a hot-wallet created, which owns the cash-notes used to build the
   unsigned transaction, create it with the corresponding secret key:
   `cargo run --release --bin safe -- wallet create <hex-encoded secret key>`

6. Use the hot-wallet to sign the built transaction:
   `cargo run --release --bin safe -- wallet sign <unsigned transaction>`

7. Copy the signed Tx generated by the above command, and send it out-of-band back to the online
   device.

Steps on the online device/computer with the watch-only wallet:

8. Broadcast the signed transaction to the network using the watch-only wallet:
   `cargo run --release --bin safe -- wowallet broadcast <signed transaction>`

9. Deposit the change cash-note to the watch-only wallet:
   `cargo run --release --bin safe -- wowallet deposit <hex-encoded public key> <change cash-note>`

10. Send/share the output cash-note generated by the above command at step #8 to/with the
    recipient.

### Auditing

We can verify a spend, optionally going back to the genesis transaction:

```
cargo run --bin safe --features local-discovery -- wallet verify [--genesis] [spend address]
```

All spends from genesis can be audited:

```
cargo run --bin safe --features local-discovery -- wallet audit
```

### Registers

Registers are one of the network's data types. The workspace here has an example app demonstrating
their use by two users to exchange text messages in a crude chat application.

In the first terminal, using the registers example, Alice creates a register:

```
cargo run --example registers --features=local-discovery -- --user alice --reg-nickname myregister
```

Alice can now write a message to the register and see anything written by anyone else. For example
she might enter the text "hello, who's there?" which is written to the register and then shown as
the "Latest value", in her terminal:

```
Register address: "50f4c9d55aa1f4fc19149a86e023cd189e509519788b4ad8625a1ce62932d1938cf4242e029cada768e7af0123a98c25973804d84ad397ca65cb89d6580d04ff07e5b196ea86f882b925be6ade06fc8d"
Register owned by: PublicKey(0cf4..08a5)
Register permissions: Permissions { anyone_can_write: true, writers: {PublicKey(0cf4..08a5)} }

Current total number of items in Register: 0
Latest value (more than one if concurrent writes were made):
--------------
--------------

Enter a blank line to receive updates, or some text to be written.
hello, who's there?
Writing msg (offline) to Register: 'hello, who's there?'
Syncing with SAFE in 2s...
synced!

Current total number of items in Register: 1
Latest value (more than one if concurrent writes were made):
--------------
[alice]: hello, who's there?
--------------

Enter a blank line to receive updates, or some text to be written.

```

For anyone else to write to the same register they need to know its xor address, so to communicate
with her friend Bob, Alice needs to find a way to send it to Bob. In her terminal, this is the
value starting "50f4..." in the output above. This value it will be different each time you run the
example to create a register.

Having received the xor address, in another terminal Bob can access the same register to see the
message Alice has written, and he can write back by running this command with the address received
from Alice. (Note that the command should all be on one line):

```
cargo run --example registers --features=local-discovery -- --user bob --reg-address 50f4c9d55aa1f4fc19149a86e023cd189e509519788b4ad8625a1ce62932d1938cf4242e029cada768e7af0123a98c25973804d84ad397ca65cb89d6580d04ff07e5b196ea86f882b925be6ade06fc8d
```

After retrieving the register and displaying the message from Alice, Bob can reply and at any time,
Alice or Bob can send another message and see any new messages which have been written, or enter a
blank line to poll for updates.

Here's Bob writing from his terminal:

```
Latest value (more than one if concurrent writes were made):
--------------
[alice]: hello, who's there?
--------------

Enter a blank line to receive updates, or some text to be written.
hi Alice, this is Bob!
```

Alice will see Bob's message when she either enters a blank line or writes another message herself.

### Inspect a Register

A second example, `register_inspect` allows you to view its structure and content. To use this with
the above example you again provide the address of the register. For example:

```
cargo run --example register_inspect --features=local-discovery -- --reg-address 50f4c9d55aa1f4fc19149a86e023cd189e509519788b4ad8625a1ce62932d1938cf4242e029cada768e7af0123a98c25973804d84ad397ca65cb89d6580d04ff07e5b196ea86f882b925be6ade06fc8d
```

After printing a summary of the register, this example will display
the structure of the register each time you press Enter, including the following:

```
Enter a blank line to print the latest register structure (or 'Q' <Enter> to quit)

Syncing with SAFE...
synced!
======================
Root (Latest) Node(s):
[ 0] Node("4eadd9"..) Entry("[alice]: this is alice 3")
[ 3] Node("f05112"..) Entry("[bob]: this is bob 3")
======================
Register Structure:
(In general, earlier nodes are more indented)
[ 0] Node("4eadd9"..) Entry("[alice]: this is alice 3")
  [ 1] Node("f5afb2"..) Entry("[alice]: this is alice 2")
    [ 2] Node("7693eb"..) Entry("[alice]: hello this is alice")
[ 3] Node("f05112"..) Entry("[bob]: this is bob 3")
  [ 4] Node("8c3cce"..) Entry("[bob]: this is bob 2")
    [ 5] Node("c7f9fc"..) Entry("[bob]: this is bob 1")
    [ 1] Node("f5afb2"..) Entry("[alice]: this is alice 2")
      [ 2] Node("7693eb"..) Entry("[alice]: hello this is alice")
======================
```

Each increase in indentation shows the children of the node above.
The numbers in square brackets are just to make it easier to see
where a node occurs more than once.

### RPC

The node manager launches each node process with a remote procedure call (RPC) service. The
workspace has a client binary that can be used to run commands against these services.

Run the `status` command with the `--details` flag to get the RPC port for each node:

```
$ cargo run --bin safenode-manager -- status --details
...
===================================
safenode-local25 - RUNNING
===================================
Version: 0.103.21
Peer ID: 12D3KooWJ4Yp8CjrbuUyeLDsAgMfCb3GAYMoBvJCRp1axjHr9cf8
Port: 38835
RPC Port: 34416
Multiaddr: /ip4/127.0.0.1/udp/38835/quic-v1/p2p/12D3KooWJ4Yp8CjrbuUyeLDsAgMfCb3GAYMoBvJCRp1axjHr9cf8
PID: 62369
Data path: /home/<<user_directory>>/.local/share/safe/node/12D3KooWJ4Yp8CjrbuUyeLDsAgMfCb3GAYMoBvJCRp1axjHr9cf8
Log path: /home/<<user_directory>>/.local/share/safe/node/12D3KooWJ4Yp8CjrbuUyeLDsAgMfCb3GAYMoBvJCRp1axjHr9cf8/logs
Bin path: target/release/safenode
Connected peers: 24
```

Now you can run RPC commands against any node.

The `info` command will retrieve basic information about the node:

```
$ cargo run --bin safenode_rpc_client -- 127.0.0.1:34416 info
Node info:
==========
RPC endpoint: https://127.0.0.1:34416
Peer Id: 12D3KooWJ4Yp8CjrbuUyeLDsAgMfCb3GAYMoBvJCRp1axjHr9cf8
Logs dir: /home/<<user_directory>>/.local/share/safe/node/12D3KooWJ4Yp8CjrbuUyeLDsAgMfCb3GAYMoBvJCRp1axjHr9cf8/logs
PID: 62369
Binary version: 0.103.21
Time since last restart: 1614s
```

The `netinfo` command will return connected peers and listeners:

```
$ cargo run --bin safenode_rpc_client -- 127.0.0.1:34416 netinfo
Node's connections to the Network:

Connected peers:
Peer: 12D3KooWJkD2pB2WdczBJWt4ZSAWfFFMa8FHe6w9sKvH2mZ6RKdm
Peer: 12D3KooWRNCqFYX8dJKcSTAgxcy5CLMcEoM87ZSzeF43kCVCCFnc
Peer: 12D3KooWLDUFPR2jCZ88pyYCNMZNa4PruweMsZDJXUvVeg1sSMtN
Peer: 12D3KooWC8GR5NQeJwTsvn9SKChRZqJU8XS8ZzKPwwgBi63FHdUQ
Peer: 12D3KooWJGERJnGd5N814V295zq1CioxUUWKgNZy4zJmBLodAPEj
Peer: 12D3KooWJ9KHPwwiRpgxwhwsjCiHecvkr2w3JsUQ1MF8q9gzWV6U
Peer: 12D3KooWSBafke1pzz3KUXbH875GYcMLVqVht5aaXNSRtbie6G9g
Peer: 12D3KooWJtKc4C7SRkei3VURDpnsegLUuQuyKxzRpCtsJGhakYfX
Peer: 12D3KooWKg8HsTQ2XmBVCeGxk7jHTxuyv4wWCWE2pLPkrhFHkwXQ
Peer: 12D3KooWQshef5sJy4rEhrtq2cHGagdNLCvcvMn9VXwMiLnqjPFA
Peer: 12D3KooWLfXHapVy4VV1DxWndCt3PmqkSRjFAigsSAaEnKzrtukD

Node's listeners:
Listener: /ip4/127.0.0.1/udp/38835/quic-v1
Listener: /ip4/192.168.1.86/udp/38835/quic-v1
Listener: /ip4/172.17.0.1/udp/38835/quic-v1
Listener: /ip4/172.18.0.1/udp/38835/quic-v1
Listener: /ip4/172.20.0.1/udp/38835/quic-v1
```

Node control commands:

```
$ cargo run --bin safenode_rpc_client -- 127.0.0.1:34416 restart 5000
Node successfully received the request to restart in 5s

$ cargo run --bin safenode_rpc_client -- 127.0.0.1:34416 stop 6000
Node successfully received the request to stop in 6s

$ cargo run --bin safenode_rpc_client -- 127.0.0.1:34416 update 7000
Node successfully received the request to try to update in 7s
```

NOTE: it is preferable to use the node manager to control the node rather than RPC commands.

Listening to royalty payment events:

```
$ cargo run --bin safenode_rpc_client -- 127.0.0.1:34416 transfers
Listening to transfers notifications... (press Ctrl+C to exit)

New transfer notification received for PublicKey(0c54..5952), containing 1 cash note/s.
CashNote received with UniquePubkey(PublicKey(19ee..1580)), value: 0.000000001

New transfer notification received for PublicKey(0c54..5952), containing 1 cash note/s.
CashNote received with UniquePubkey(PublicKey(19ee..1580)), value: 0.000000001
```

The `transfers` command can provide a path for royalty payment cash notes:

```
$ cargo run --release --bin=safenode_rpc_client -- 127.0.0.1:34416 transfers ./royalties-cash-notes
Listening to transfers notifications... (press Ctrl+C to exit)
Writing cash notes to: ./royalties-cash-notes
```

Each received cash note is written to a file in the directory above, under another directory
corresponding to the public address of the recipient.

### Tear Down

When you're finished experimenting, tear down the network:

```bash
cargo run --bin safenode-manager -- local kill
```

## Metrics Dashboard

Use the `open-metrics` feature flag on the node / client to start
an [OpenMetrics](https://github.com/OpenObservability/OpenMetrics/) exporter. The metrics are
served via a webserver started at a random port. Check the log file / stdout to find the webserver
URL, `Metrics server on http://127.0.0.1:xxxx/metrics`

The metrics can then be collected using a collector (for e.g. Prometheus) and the data can then be
imported into any visualization tool (for e.g., Grafana) to be further analyzed. Refer to
this [Guide](./metrics/README.md) to easily setup a dockerized Grafana dashboard to visualize the
metrics.

## Contributing

Feel free to clone and modify this project. Pull requests are welcome.<br>You can also visit *
*[The MaidSafe Forum](https://safenetforum.org/)** for discussion or if you would like to join our
online community.

### Pull Request Process

1. Please direct all pull requests to the `alpha` branch instead of the `main` branch.
1. Ensure that your commit messages clearly describe the changes you have made and use
   the [Conventional Commits](https://www.conventionalcommits.org/) specification.

## License

This Safe Network repository is licensed under the General Public License (GPL), version
3 ([LICENSE](http://www.gnu.org/licenses/gpl-3.0.en.html)).
