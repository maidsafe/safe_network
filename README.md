# stableset_net

This is an experimental branch for layering safe network data structures atop libp2p

## Running the network

`killall safenode || true && RUST_LOG=safenode,sn_node cargo run --bin testnet -- -b --interval 100`

- Put files
`cargo run --release --bin safenode -- --upload-chunks ~/dir/with/files`

- Get files; copy the `XorName` of the file from the previous command
`cargo run --release --bin safenode -- --get-chunk xor_name`

### Notes

- Currently we've pulled in testnet bin from the main `sn` repo for ease of spinning up nodes.
- Logs are output to the standard `~/.safe/node/local-test-network` dir.


### TODO

- [ ] Basic messaging to target nodes
- [ ] Add RPC for simplest node/net interaction (do libp2p CLIs help here?)
- [ ] Add in chunking etc
- [ ] Add in DBCs and validation handling
