mod error;

pub use crate::error::{Error, Result};

use async_trait::async_trait;
use libp2p::kad::RecordKey;
use libp2p::{Multiaddr, PeerId};
use sn_protocol::safenode_proto::{
    safe_node_client::SafeNodeClient, GossipsubPublishRequest, GossipsubSubscribeRequest,
    GossipsubUnsubscribeRequest, NetworkInfoRequest, NodeInfoRequest, RecordAddressesRequest,
    RestartRequest, StopRequest, UpdateRequest,
};
use std::path::PathBuf;
use std::str::FromStr;
use tokio::time::Duration;
use tonic::Request;

#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub pid: u32,
    pub peer_id: PeerId,
    pub log_path: PathBuf,
    pub data_path: PathBuf,
    pub version: String,
    pub uptime: Duration,
}

#[derive(Debug, Clone)]
pub struct NetworkInfo {
    pub connected_peers: Vec<PeerId>,
    pub listeners: Vec<Multiaddr>,
}

#[derive(Debug, Clone)]
pub struct RecordAddress {
    pub key: RecordKey,
}

#[async_trait]
pub trait RpcActions {
    async fn node_info(&self) -> Result<NodeInfo>;
    async fn network_info(&self) -> Result<NetworkInfo>;
    async fn record_addresses(&self) -> Result<Vec<RecordAddress>>;
    async fn gossipsub_subscribe(&self, topic: &str) -> Result<()>;
    async fn gossipsub_unsubscribe(&self, topic: &str) -> Result<()>;
    async fn gossipsub_publish(&self, topic: &str, message: &str) -> Result<()>;
    async fn node_restart(&self, delay_millis: u64) -> Result<()>;
    async fn node_stop(&self, delay_millis: u64) -> Result<()>;
    async fn node_update(&self, delay_millis: u64) -> Result<()>;
}

pub struct RpcClient {
    endpoint: String,
}

impl RpcClient {
    pub fn new(endpoint: &str) -> RpcClient {
        RpcClient {
            endpoint: endpoint.to_string(),
        }
    }
}

#[async_trait]
impl RpcActions for RpcClient {
    async fn node_info(&self) -> Result<NodeInfo> {
        let mut client = SafeNodeClient::connect(self.endpoint.clone()).await?;
        let response = client.node_info(Request::new(NodeInfoRequest {})).await?;
        let node_info_resp = response.get_ref();
        let peer_id = PeerId::from_bytes(&node_info_resp.peer_id)?;
        let node_info = NodeInfo {
            pid: node_info_resp.pid,
            peer_id,
            log_path: PathBuf::from(node_info_resp.log_dir.clone()),
            data_path: PathBuf::from(node_info_resp.data_dir.clone()),
            version: node_info_resp.bin_version.clone(),
            uptime: Duration::from_secs(node_info_resp.uptime_secs),
        };
        Ok(node_info)
    }

    async fn network_info(&self) -> Result<NetworkInfo> {
        let mut client = SafeNodeClient::connect(self.endpoint.clone()).await?;
        let response = client
            .network_info(Request::new(NetworkInfoRequest {}))
            .await?;
        let network_info = response.get_ref();

        let mut connected_peers = Vec::new();
        for bytes in network_info.connected_peers.iter() {
            let peer_id = PeerId::from_bytes(bytes)?;
            connected_peers.push(peer_id);
        }

        let mut listeners = Vec::new();
        for multiaddr_str in network_info.listeners.iter() {
            let multiaddr = Multiaddr::from_str(multiaddr_str)?;
            listeners.push(multiaddr);
        }

        Ok(NetworkInfo {
            connected_peers,
            listeners,
        })
    }

    async fn record_addresses(&self) -> Result<Vec<RecordAddress>> {
        let mut client = SafeNodeClient::connect(self.endpoint.clone()).await?;
        let response = client
            .record_addresses(Request::new(RecordAddressesRequest {}))
            .await?;
        let mut record_addresses = vec![];
        for bytes in response.get_ref().addresses.iter() {
            let key = libp2p::kad::RecordKey::from(bytes.clone());
            record_addresses.push(RecordAddress { key });
        }
        Ok(record_addresses)
    }

    async fn gossipsub_subscribe(&self, topic: &str) -> Result<()> {
        let mut client = SafeNodeClient::connect(self.endpoint.clone()).await?;
        let _response = client
            .subscribe_to_topic(Request::new(GossipsubSubscribeRequest {
                topic: topic.to_string(),
            }))
            .await?;
        Ok(())
    }

    async fn gossipsub_unsubscribe(&self, topic: &str) -> Result<()> {
        let mut client = SafeNodeClient::connect(self.endpoint.clone()).await?;
        let _response = client
            .unsubscribe_from_topic(Request::new(GossipsubUnsubscribeRequest {
                topic: topic.to_string(),
            }))
            .await?;
        Ok(())
    }

    async fn gossipsub_publish(&self, topic: &str, msg: &str) -> Result<()> {
        let mut client = SafeNodeClient::connect(self.endpoint.clone()).await?;
        let _response = client
            .publish_on_topic(Request::new(GossipsubPublishRequest {
                topic: topic.to_string(),
                msg: msg.into(),
            }))
            .await?;
        Ok(())
    }

    async fn node_restart(&self, delay_millis: u64) -> Result<()> {
        let mut client = SafeNodeClient::connect(self.endpoint.clone()).await?;
        let _response = client
            .restart(Request::new(RestartRequest { delay_millis }))
            .await?;
        Ok(())
    }

    async fn node_stop(&self, delay_millis: u64) -> Result<()> {
        let mut client = SafeNodeClient::connect(self.endpoint.clone()).await?;
        let _response = client
            .stop(Request::new(StopRequest { delay_millis }))
            .await?;
        Ok(())
    }

    async fn node_update(&self, delay_millis: u64) -> Result<()> {
        let mut client = SafeNodeClient::connect(self.endpoint.clone()).await?;
        let _response = client
            .update(Request::new(UpdateRequest { delay_millis }))
            .await?;
        Ok(())
    }
}
