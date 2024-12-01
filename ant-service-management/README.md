# Service Management

Provides utilities for dealing with services, which are mainly used by the node manager.

## RPC Actions

The `RpcActions` trait defines the protocol that is currently available for interacting with `antnode`:

```
node_info: Returns information about the node, such as its peer ID and version.
network_info: Retrieves network-related information, such as the peers currently connected to the node.
record_addresses: Provides a list of the node's record addresses.
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
