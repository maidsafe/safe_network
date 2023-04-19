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
`cargo run --release --bin safe -- files download ~/dir/with/files/uploaded_files`

Note that the names of the uploaded files will be inserted into a new file, which is placed
in a new dir named `uploaded_files` in the dir you gave when uploading the files. The names found in that new file will then be used to download the uploaded files.

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
Node info received:
===================
RPC endpoint: http://127.0.0.1:12001
Node Id: 65fbed(01100101)..
Logs dir: /home/bochaco/.safe/node/local-test-network/safenode-1
Binary version: 0.1.0
Time since last restart: 650s
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
