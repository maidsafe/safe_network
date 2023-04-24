# The Safe Network

This is the Safe Network as it was supposed to be, on a kademlia network, enabled by libp2p.

## Running the network

`killall safenode || true && RUST_LOG=safenode,safe cargo run --bin testnet -- -b --interval 100`

## Actions undertaken by a client accessing the network

- Create Register with name 'myregister':
`cargo run --release --bin safe -- register create myregister`

- Get Register using its name from the previous cmd:
`cargo run --release --bin safe -- register get myregister`

- Edit Register using its name from the previous cmd:
`cargo run --release --bin safe -- register edit myregister somename`

- Upload files
`cargo run --release --bin safe -- files upload ~/dir/with/files`

- Download files
`cargo run --release --bin safe -- files download`

Note that the names of the uploaded files will be inserted into a new text document with a file 
name of `file_names_%Y-%m-%d_%H-%M-%S.txt` (i.e. unique by date and time of upload) which is placed in `$HOME/.safe/client/uploaded_files`. 
When calling `files download`, the `uploaded_files` dir will be searched for documents containing the names of uploaded files.
If you don't wish to download the same files multiple times, remove the text documents after the first download.

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
$ cargo run --release --example safenode_rpc_client -- 127.0.0.1:12001 info
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
$ cargo run --release --example safenode_rpc_client -- 127.0.0.1:12001 netinfo
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
$ cargo run --release --example safenode_rpc_client -- 127.0.0.1:12001 restart 5000
Node successfully received the request to restart in 5s

$ cargo run --release --example safenode_rpc_client -- 127.0.0.1:12001 stop 6000
Node successfully received the request to stop in 6s

$ cargo run --release --example safenode_rpc_client -- 127.0.0.1:12001 update 7000
Node successfully received the request to try to update in 7s
```
### Notes

- Currently we've pulled in testnet bin from the main `sn` repo for ease of spinning up nodes.
- Logs are output to the standard `~/.safe/node/local-test-network` dir.


### TODO

- [ ] Add RPC for simplest node/net interaction (do libp2p CLIs help here?)



### Archive

The elder-membership agreed, section tree backed implementation of the safe network can be found [here](https://github.com/maidsafe/safe_network_archive)
