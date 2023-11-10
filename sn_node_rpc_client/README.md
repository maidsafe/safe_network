# Safenode RPC Client

This crate provides a client for the RPC protocol for interacting with `safenode`. It wraps the Protobuf-generated code and types such that users of the RPC protocol don't need to redefine that code.

It also provides a binary which is a CLI for interacting with a running `safenode` instance via the protocol.

## RPC Actions

The `RpcActions` trait defines the protocol that is currently available for interacting with `safenode`:
```
node_info: Returns information about the node, such as its peer ID and version.
network_info: Retrieves network-related information, such as the peers currently connected to the node.
record_addresses: Provides a list of the node's record addresses.
gossipsub_subscribe: Subscribes to a specific topic on the gossipsub network.
gossipsub_unsubscribe: Unsubscribes from a given topic on the gossipsub network.
gossipsub_publish: Publishes a message to a specified topic on the gossipsub network.
restart_node: Requests the node to restart.
stop_node: Requests the node to stop its operations.
update_node: Updates the node with provided parameters.
```

Users of the crate can program against the trait rather than the `RpcClient` implementation.

This can facilitate behaviour-based unit testing, like so:
```
use mockall::mock;
use mockall::predicate::*;

mock! {
    pub RpcClient {}
    #[async_trait]
    impl RpcClientInterface for RpcClient {
        async fn node_info(&self) -> RpcResult<NodeInfo>;
        async fn network_info(&self) -> RpcResult<NetworkInfo>;
        async fn record_addresses(&self) -> Result<Vec<RecordAddress>>;
        async fn gossipsub_subscribe(&self, topic: &str) -> Result<()>;
        async fn gossipsub_unsubscribe(&self, topic: &str) -> Result<()>;
        async fn gossipsub_publish(&self, topic: &str, message: &str) -> Result<()>;
        async fn node_restart(&self, delay_millis: u64) -> Result<()>;
        async fn node_stop(&self, delay_millis: u64) -> Result<()>;
        async fn node_update(&self, delay_millis: u64) -> Result<()>;
    }
}
```

## Binary Usage

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
