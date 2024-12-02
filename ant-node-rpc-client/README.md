# Antnode RPC Client

This crate provides a client for the RPC protocol for interacting with `antnode`. It wraps the Protobuf-generated code and types such that users of the RPC protocol don't need to redefine that code.

It also provides a binary which is a CLI for interacting with a running `antnode` instance via the protocol.

## Binary Usage

Run `cargo run -- <ADDR> <command>` to connect to a node. Provide the address of the node's RPC service, e.g. 127.0.0.1:12001. Followed by the command to execute. Some of the commands available are:

- `info`: Retrieve information about the node itself
- `netinfo`: Retrieve information about the node's connections to the network
- `events`: Start listening for node events
- `transfers`: Start listening for transfers events
- `restart`: Restart the node after the specified delay
- `stop`: Stop the node after the specified delay
- `update`: Update to latest `antnode` released version, and restart it

For more information about each command, run `cargo run -- <command> --help`.
