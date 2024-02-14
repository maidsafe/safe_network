// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use bls::{PublicKey, PK_SIZE};
use bytes::Bytes;
use eyre::{ErrReport, Result};
use sn_node::RunningNode;
use sn_protocol::node_rpc::NodeCtrl;
use sn_protocol::safenode_proto::{
    k_buckets_response,
    safe_node_server::{SafeNode, SafeNodeServer},
    GossipsubPublishRequest, GossipsubPublishResponse, GossipsubSubscribeRequest,
    GossipsubSubscribeResponse, GossipsubUnsubscribeRequest, GossipsubUnsubscribeResponse,
    KBucketsRequest, KBucketsResponse, NetworkInfoRequest, NetworkInfoResponse, NodeEvent,
    NodeEventsRequest, NodeInfoRequest, NodeInfoResponse, RecordAddressesRequest,
    RecordAddressesResponse, RestartRequest, RestartResponse, StopRequest, StopResponse,
    TransferNotifsFilterRequest, TransferNotifsFilterResponse, UpdateRequest, UpdateResponse,
};
use std::{
    collections::HashMap,
    env,
    net::SocketAddr,
    process,
    time::{Duration, Instant},
};
use tokio::sync::mpsc::{self, Sender};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Code, Request, Response, Status};
use tracing::{debug, info};

// Defining a struct to hold information used by our gRPC service backend
struct SafeNodeRpcService {
    addr: SocketAddr,
    log_dir: String,
    running_node: RunningNode,
    ctrl_tx: Sender<NodeCtrl>,
    started_instant: Instant,
}

// Implementing RPC interface for service defined in .proto
#[tonic::async_trait]
impl SafeNode for SafeNodeRpcService {
    type NodeEventsStream = ReceiverStream<Result<NodeEvent, Status>>;

    async fn node_info(
        &self,
        request: Request<NodeInfoRequest>,
    ) -> Result<Response<NodeInfoResponse>, Status> {
        debug!(
            "RPC request received at {}: {:?}",
            self.addr,
            request.get_ref()
        );

        let resp = Response::new(NodeInfoResponse {
            peer_id: self.running_node.peer_id().to_bytes(),
            log_dir: self.log_dir.clone(),
            data_dir: self
                .running_node
                .root_dir_path()
                .to_string_lossy()
                .to_string(),
            pid: process::id(),
            bin_version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_secs: self.started_instant.elapsed().as_secs(),
            wallet_balance: self
                .running_node
                .get_node_wallet_balance()
                .expect("Failed to get node wallet balance")
                .as_nano(),
        });

        Ok(resp)
    }

    async fn network_info(
        &self,
        request: Request<NetworkInfoRequest>,
    ) -> Result<Response<NetworkInfoResponse>, Status> {
        debug!(
            "RPC request received at {}: {:?}",
            self.addr,
            request.get_ref()
        );

        let state = self
            .running_node
            .get_swarm_local_state()
            .await
            .expect("failed to get local swarm state");
        let connected_peers = state.connected_peers.iter().map(|p| p.to_bytes()).collect();
        let listeners = state.listeners.iter().map(|m| m.to_string()).collect();

        let resp = Response::new(NetworkInfoResponse {
            connected_peers,
            listeners,
        });

        Ok(resp)
    }

    async fn node_events(
        &self,
        request: Request<NodeEventsRequest>,
    ) -> Result<Response<Self::NodeEventsStream>, Status> {
        debug!(
            "RPC request received at {}: {:?}",
            self.addr,
            request.get_ref()
        );

        let (client_tx, client_rx) = mpsc::channel(4);

        let mut events_rx = self.running_node.node_events_channel().subscribe();
        let _handle = tokio::spawn(async move {
            while let Ok(event) = events_rx.recv().await {
                let event_bytes = match event.to_bytes() {
                    Ok(bytes) => bytes,
                    Err(err) => {
                        debug!(
                            "Error {err:?} while converting NodeEvent to bytes, ignoring the error"
                        );
                        continue;
                    }
                };

                let event = NodeEvent { event: event_bytes };

                if let Err(err) = client_tx.send(Ok(event)).await {
                    debug!(
                        "Dropping stream sender to RPC client due to failure in \
                        last attempt to notify an event: {err}"
                    );
                    break;
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(client_rx)))
    }

    async fn transfer_notifs_filter(
        &self,
        request: Request<TransferNotifsFilterRequest>,
    ) -> Result<Response<TransferNotifsFilterResponse>, Status> {
        debug!(
            "RPC request received at {}: {:?}",
            self.addr,
            request.get_ref()
        );

        let mut pk_bytes = [0u8; PK_SIZE];
        pk_bytes.copy_from_slice(&request.get_ref().pk);
        let pk = match PublicKey::from_bytes(pk_bytes) {
            Ok(pk) => pk,
            Err(err) => {
                return Err(Status::new(
                    Code::Internal,
                    format!("Failed to decode provided pk: {err}"),
                ))
            }
        };

        match self.running_node.transfer_notifs_filter(Some(pk)) {
            Ok(()) => Ok(Response::new(TransferNotifsFilterResponse {})),
            Err(err) => Err(Status::new(
                Code::Internal,
                format!("Failed to set transfer notifs filter with {pk:?}: {err}"),
            )),
        }
    }

    async fn record_addresses(
        &self,
        request: Request<RecordAddressesRequest>,
    ) -> Result<Response<RecordAddressesResponse>, Status> {
        debug!(
            "RPC request received at {}: {:?}",
            self.addr,
            request.get_ref()
        );

        let addresses = self
            .running_node
            .get_all_record_addresses()
            .await
            .expect("failed to get record addresses")
            .into_iter()
            .map(|addr| addr.as_bytes())
            .collect();

        Ok(Response::new(RecordAddressesResponse { addresses }))
    }

    async fn k_buckets(
        &self,
        request: Request<KBucketsRequest>,
    ) -> Result<Response<KBucketsResponse>, Status> {
        debug!(
            "RPC request received at {}: {:?}",
            self.addr,
            request.get_ref()
        );

        let kbuckets: HashMap<u32, k_buckets_response::Peers> = self
            .running_node
            .get_kbuckets()
            .await
            .expect("failed to get k-buckets")
            .into_iter()
            .map(|(ilog2_distance, peers)| {
                let peers = peers.into_iter().map(|peer| peer.to_bytes()).collect();
                let peers = k_buckets_response::Peers { peers };
                (ilog2_distance, peers)
            })
            .collect();

        Ok(Response::new(KBucketsResponse { kbuckets }))
    }

    async fn subscribe_to_topic(
        &self,
        request: Request<GossipsubSubscribeRequest>,
    ) -> Result<Response<GossipsubSubscribeResponse>, Status> {
        debug!(
            "RPC request received at {}: {:?}",
            self.addr,
            request.get_ref()
        );

        let topic = &request.get_ref().topic;

        // Assuming the rpc subscription request also force the node to handle the gossip.
        // So far, this is only used during test to allow counting the gossip msgs received by node.
        if let Err(err) = self.running_node.start_handle_gossip() {
            return Err(Status::new(
                Code::Internal,
                format!("Failed to start handle gossip: {err}"),
            ));
        }

        match self.running_node.subscribe_to_topic(topic.clone()) {
            Ok(()) => Ok(Response::new(GossipsubSubscribeResponse {})),
            Err(err) => Err(Status::new(
                Code::Internal,
                format!("Failed to subscribe to topic '{topic}': {err}"),
            )),
        }
    }

    async fn unsubscribe_from_topic(
        &self,
        request: Request<GossipsubUnsubscribeRequest>,
    ) -> Result<Response<GossipsubUnsubscribeResponse>, Status> {
        debug!(
            "RPC request received at {}: {:?}",
            self.addr,
            request.get_ref()
        );

        let topic = &request.get_ref().topic;

        match self.running_node.unsubscribe_from_topic(topic.clone()) {
            Ok(()) => Ok(Response::new(GossipsubUnsubscribeResponse {})),
            Err(err) => Err(Status::new(
                Code::Internal,
                format!("Failed to unsubscribe from topic '{topic}': {err}"),
            )),
        }
    }

    async fn publish_on_topic(
        &self,
        request: Request<GossipsubPublishRequest>,
    ) -> Result<Response<GossipsubPublishResponse>, Status> {
        debug!(
            "RPC request received at {}: {:?}",
            self.addr,
            request.get_ref()
        );

        let topic = &request.get_ref().topic;
        // Convert the message from Vec<u8> to Bytes
        let msg = Bytes::from(request.get_ref().msg.clone());

        match self.running_node.publish_on_topic(topic.clone(), msg) {
            Ok(()) => Ok(Response::new(GossipsubPublishResponse {})),
            Err(err) => Err(Status::new(
                Code::Internal,
                format!("Failed to publish on topic '{topic}': {err}"),
            )),
        }
    }

    async fn stop(&self, request: Request<StopRequest>) -> Result<Response<StopResponse>, Status> {
        debug!(
            "RPC request received at {}: {:?}",
            self.addr,
            request.get_ref()
        );

        let cause = if let Some(addr) = request.remote_addr() {
            ErrReport::msg(format!(
                "Node has been stopped by an RPC request from {addr}."
            ))
        } else {
            ErrReport::msg("Node has been stopped by an RPC request from an unknown address.")
        };

        let delay = Duration::from_millis(request.get_ref().delay_millis);
        match self.ctrl_tx.send(NodeCtrl::Stop { delay, cause }).await {
            Ok(()) => Ok(Response::new(StopResponse {})),
            Err(err) => Err(Status::new(
                Code::Internal,
                format!("Failed to stop the node: {err}"),
            )),
        }
    }

    async fn restart(
        &self,
        request: Request<RestartRequest>,
    ) -> Result<Response<RestartResponse>, Status> {
        debug!(
            "RPC request received at {}: {:?}",
            self.addr,
            request.get_ref()
        );

        let delay = Duration::from_millis(request.get_ref().delay_millis);
        match self.ctrl_tx.send(NodeCtrl::Restart(delay)).await {
            Ok(()) => Ok(Response::new(RestartResponse {})),
            Err(err) => Err(Status::new(
                Code::Internal,
                format!("Failed to restart the node: {err}"),
            )),
        }
    }

    async fn update(
        &self,
        request: Request<UpdateRequest>,
    ) -> Result<Response<UpdateResponse>, Status> {
        debug!(
            "RPC request received at {}: {:?}",
            self.addr,
            request.get_ref()
        );

        let delay = Duration::from_millis(request.get_ref().delay_millis);
        match self.ctrl_tx.send(NodeCtrl::Update(delay)).await {
            Ok(()) => Ok(Response::new(UpdateResponse {})),
            Err(err) => Err(Status::new(
                Code::Internal,
                format!("Failed to update the node: {err}"),
            )),
        }
    }
}

pub(crate) fn start_rpc_service(
    addr: SocketAddr,
    log_dir_path: &str,
    running_node: RunningNode,
    ctrl_tx: Sender<NodeCtrl>,
    started_instant: Instant,
) {
    // creating a service
    let service = SafeNodeRpcService {
        addr,
        log_dir: log_dir_path.to_string(),
        running_node,
        ctrl_tx,
        started_instant,
    };
    info!("RPC Server listening on {addr}");
    println!("RPC Server listening on {addr}");

    let _handle = tokio::spawn(async move {
        // adding our service to our server.
        if let Err(e) = Server::builder()
            .add_service(SafeNodeServer::new(service))
            .serve(addr)
            .await
        {
            error!("RPC Server failed to start: {e:?}");
        }
    });
}
