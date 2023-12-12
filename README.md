# The Safe Network

This is the Safe Network as it was supposed to be, on a kademlia network, enabled by libp2p.

## Running the network

You'll need to set the `SAFE_PEERS` env variable to the multiaddress of a node when you set up a testnet.
You can do this automatically at network startup using the following command (if you have ripgrep installed)
```bash
killall safenode || true && RUST_LOG=safenode,safe cargo run --bin testnet -- --build-node --build-faucet --interval 100  && export SAFE_PEERS=$(rg "listening on \".+\"" ~/.safe -u | rg '/ip4.*$' -m1 -o | rg '"' -r '')
```

This will set the env var for you and so you can run the client without needing to manually pass in `--peer` args.

Or alternatively run with local discovery enabled (mDNS)
`killall safenode || true && RUST_LOG=safenode,safe cargo run --bin testnet --features local-discovery -- --build-node --build-faucet --interval 100`

## Actions undertaken by a client accessing the network

Assuming you have `SAFE_PEERS` set as above:

- Create Register with name 'myregister':
`cargo run --release --bin safe -- register create myregister`

- Get Register using its name from the previous cmd:
`cargo run --release --bin safe -- register get -n myregister`

- Edit Register using its name from the previous cmd:
`cargo run --release --bin safe -- register edit -n myregister somename`

- Upload files
`cargo run --release --bin safe -- files upload ~/dir/with/files`

- Download files
`cargo run --release --bin safe -- files download`

Note that the names of the uploaded files will be inserted into a new text document with a file 
name of `file_names_%Y-%m-%d_%H-%M-%S.txt` (i.e. unique by date and time of upload) which is placed in `$HOME/.safe/client/uploaded_files`. 
When calling `files download`, the `uploaded_files` dir will be searched for documents containing the names of uploaded files.
If you don't wish to download the same files multiple times, remove the text documents after the first download.

If you don't have `SAFE_PEERS` set, you can pass in a `--peer` argument to commands like this:
`cargo run --release --bin safe -- --peer <multiaddress> files upload ~/dir/with/files`

## Local Faucet

Start a local network as described above.

First we need to claim the genesis, which means all available supply is sent to a faucet wallet located in the `$HOME/.safe/test_faucet` path.
Thereafter we can ask the faucet to send tokens to any address we specify.
To get the address of your wallet, just call the address cmd. It will create the wallet if it doesn't exist.

- Claim genesis
`cargo run --release --bin faucet -- claim-genesis`

- Start a local faucet server
`cargo run --release --bin faucet -- server`

- Request tokens from the local faucet server (requires a running faucet server)
`cargo run --release --bin safe wallet get-faucet http://localhost:8000`

Please note that this feature is still unstable and most likely won't work yet.

## Token transfers

Start a local network and a faucet as described above.

- Get your wallet address
`cargo run --release --bin safe -- wallet address`

- Send tokens to an address, this will output a Transfer hex string that you must send to the recipient out-of-band
`cargo run --release --bin safe -- wallet send [amount] [address]`

- Receive tokens from a Transfer hex string
`cargo run --release --bin safe -- wallet receive [transfer]`

## Auditing

- Verify a Spend on the Network (optionally recursively all the way back to genesis)
`cargo run --release --bin safe -- wallet verify [--genesis] [spend address]`

- Audit all the Spends on the Network from Genesis
`cargo run --release --bin safe -- wallet audit`

## Using example app which exercises the Register APIs

You can run the `registers` example client app from multiple consoles simultaneously,
to write to the same Register on the network, identified by its nickname and
using different user names from each instance launched, e.g.:

From first console:
```
cargo run --release --example registers -- --user alice --reg-nickname myregister
```

From a second console:
```
cargo run --release --example registers -- --user bob --reg-nickname myregister
```

## Using the example RPC client app to query info and send cmds to a running safenode

- Query basic node info
```
$ cargo run --release --bin=safenode_rpc_client -- 127.0.0.1:12001 info
Node info:
===================
RPC endpoint: http://127.0.0.1:12001
Peer Id: 12D3KooWB5CXPPtbVzZ7K9dv8xLj4JAPVEQu7ehibs2bWrqwiowy
Logs dir: /home/bochaco/.safe/node/local-test-network/safenode-1
PID: 490955
Binary version: 0.1.0
Time since last restart: 650s
```

- Query info about node's connections to the network:
```
$ cargo run --release --bin=safenode_rpc_client -- 127.0.0.1:12001 netinfo
Node's connections to the Network:

Connected peers:
Peer: 12D3KooWCRN4jQjyACrHq4mAq1ZLDDnA1E9cDGoGuXP1pZbRDJee
Peer: 12D3KooWFc2PX9Y7bQfUULHrg1VYeNAVKyS5mUjQJfzDy3NqSn2t
Peer: 12D3KooWA2jeb4YdkTb5zw2ajWK4zqgoVaMN5y1eDrkUCXoin94V
Peer: 12D3KooWLHZBRw47aqXCedSYvv4QQWsYdEX9HnDV6YwZBjujWAZV
Peer: 12D3KooWJUExWkuqProAgTBhABMeQoi25zBpqdmGEncs1X62NCtV
Peer: 12D3KooWENu5uDQsSdb4XCVeLZhXav922uyWHnyfLFwC5KZGKrpR
Peer: 12D3KooWSaEKWKPGh5Q3fQrn6xqsyvQsKT2y5XxxZXjCqQbP35eE
Peer: 12D3KooWNCvmBaz1MkByYkYArxKVQdiCA4bKDDBgFBtBzcpfDwA5
Peer: 12D3KooWJPkWZHnsqwwHCWXj5MV3MaoNXksTKRGMNjAcaqydYKRv

Node's listeners:
Listener: /ip4/127.0.0.1/udp/47117/quic-v1
Listener: /ip4/192.168.0.155/udp/47117/quic-v1
Listener: /ip4/172.17.0.1/udp/47117/quic-v1
```

- Restarting/Updating/Stopping a node
```
$ cargo run --release --bin=safenode_rpc_client -- 127.0.0.1:12001 restart 5000
Node successfully received the request to restart in 5s

$ cargo run --release --bin=safenode_rpc_client -- 127.0.0.1:12001 stop 6000
Node successfully received the request to stop in 6s

$ cargo run --release --bin=safenode_rpc_client -- 127.0.0.1:12001 update 7000
Node successfully received the request to try to update in 7s
```

- Listening to network royalties payments events
```
$ cargo run --release --bin=safenode_rpc_client -- 127.0.0.1:12001 transfers
Listening to transfers notifications... (press Ctrl+C to exit)

New transfer notification received for PublicKey(0c54..5952), containing 1 cash note/s.
CashNote received with UniquePubkey(PublicKey(19ee..1580)), value: 0.000000001

New transfer notification received for PublicKey(0c54..5952), containing 1 cash note/s.
CashNote received with UniquePubkey(PublicKey(19ee..1580)), value: 0.000000001

```

A path to local disk where to store royalties payments cash notes received can be provided as well, e.g.:
```
$ cargo run --release --bin=safenode_rpc_client -- 127.0.0.1:12001 transfers ./royalties-cash-notes
Listening to transfers notifications... (press Ctrl+C to exit)
Writing cash notes to: ./royalties-cash-notes

```
Each CashNote is written to a separate file in respective recipient public address dir in the created cash_notes dir. Each file is named after the CashNote id.

# Metrics Dashboard

Use the `open-metrics` feature flag on the node / client to start an [OpenMetrics](https://github.com/OpenObservability/OpenMetrics/) exporter. The metrics are served via a webserver started at a random port. Check the log file / stdout to find the webserver URL, `Metrics server on http://127.0.0.1:xxxx/metrics`

The metrics can then be collected using a collector (for e.g. Prometheus) and the data can then be imported into any visualization tool (for e.g., Grafana) to be further analyzed. Refer to this [Guide](./metrics/README.md) to easily setup a dockerized Grafana dashboard to visualize the metrics.

### Archive

The elder-membership agreed, section tree backed implementation of the safe network can be found [here](https://github.com/maidsafe/safe_network_archive)
