# Safenode RPC Client
This binary provides a command line interface to interact with a running Safenode instance.

## Usage
Run `cargo run -- <ADDR> <command>` to connect to a node. Provide the address of the node's RPC service, e.g. 127.0.0.1:12001. Followed by the command to execute. Some of the commands available are:

- `info`: Retrieve information about the node itself
- `netinfo`: Retrieve information about the node's connections to the network
- `events`: Start listening for node events
- `transfers`: Start listening for transfers events
- `subscribe`: Subscribe to a given Gossipsub topic
- `unsubscribe`: Unsubscribe from a given Gossipsub topic
- `publish`: Publish a msg on a given Gossipsub topic
- `restart`: Restart the node after the specified delay
- `stop`: Stop the node after the specified delay
- `update`: Update to latest `safenode` released version, and restart it

For more information about each command, run `cargo run -- <command> --help`.
