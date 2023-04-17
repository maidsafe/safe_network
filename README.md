# stableset_net

This is the Safe Network as it was supposed to be, on a kademlia network, enabled by libp2p.

## Running the network

`killall safenode || true && RUST_LOG=safenode,safe cargo run --bin testnet -- -b --interval 100`

## Actions undertaken by a client accessing the network

- Create Register with nickname 'myregister'
`cargo run --release --bin safe -- --create-register myregister`

- Get Register using its nickname from the previous command
`cargo run --release --bin safe -- --query-register myregister`

- Put files
`cargo run --release --bin safe -- --upload-chunks ~/dir/with/files`

- Get files; copy the `XorName` of the file from the previous command
`cargo run --release --bin safe -- --get-chunk xor_name`

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

### Notes

- Currently we've pulled in testnet bin from the main `sn` repo for ease of spinning up nodes.
- Logs are output to the standard `~/.safe/node/local-test-network` dir.


### TODO

- [ ] Basic messaging to target nodes
- [ ] Add RPC for simplest node/net interaction (do libp2p CLIs help here?)
- [ ] Add in chunking etc
- [ ] Add in DBCs and validation handling
