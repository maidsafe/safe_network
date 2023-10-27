// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use bytes::Bytes;
use sn_node::RunningNode;

use super::NodeCtrl;

use eyre::{ErrReport, Result};
use std::{
    env,
    net::SocketAddr,
    process,
    time::{Duration, Instant},
};
use tokio::sync::mpsc::{self, Sender};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Code, Request, Response, Status};
use tracing::{debug, info, trace};

use safenode_proto::safe_node_server::{SafeNode, SafeNodeServer};
use safenode_proto::{
    GossipsubPublishRequest, GossipsubPublishResponse, GossipsubSubscribeRequest,
    GossipsubSubscribeResponse, GossipsubUnsubscribeRequest, GossipsubUnsubscribeResponse,
    NetworkInfoRequest, NetworkInfoResponse, NodeEvent, NodeEventsRequest, NodeInfoRequest,
    NodeInfoResponse, RecordAddressesRequest, RecordAddressesResponse, RestartRequest,
    RestartResponse, StopRequest, StopResponse, UpdateRequest, UpdateResponse,
};

// this includes code generated from .proto files
mod safenode_proto {
    tonic::include_proto!("safenode_proto");
}

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
        trace!(
            "RPC request received at {}: {:?}",
            self.addr,
            request.get_ref()
        );

        let resp = Response::new(NodeInfoResponse {
            peer_id: self.running_node.peer_id().to_bytes(),
            log_dir: self.log_dir.clone(),
            pid: process::id(),
            bin_version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_secs: self.started_instant.elapsed().as_secs(),
        });

        Ok(resp)
    }

    async fn network_info(
        &self,
        request: Request<NetworkInfoRequest>,
    ) -> Result<Response<NetworkInfoResponse>, Status> {
        trace!(
            "RPC request received at {}: {:?}",
            self.addr,
            request.get_ref()
        );

        let state = self.running_node.get_swarm_local_state().await.unwrap();
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
        trace!(
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

    async fn record_addresses(
        &self,
        request: Request<RecordAddressesRequest>,
    ) -> Result<Response<RecordAddressesResponse>, Status> {
        trace!(
            "RPC request received at {}: {:?}",
            self.addr,
            request.get_ref()
        );

        let addresses = self
            .running_node
            .get_all_record_addresses()
            .await
            .unwrap()
            .into_iter()
            .map(|addr| addr.as_bytes())
            .collect();

        Ok(Response::new(RecordAddressesResponse { addresses }))
    }

    async fn subscribe_to_topic(
        &self,
        request: Request<GossipsubSubscribeRequest>,
    ) -> Result<Response<GossipsubSubscribeResponse>, Status> {
        trace!(
            "RPC request received at {}: {:?}",
            self.addr,
            request.get_ref()
        );

        let topic = &request.get_ref().topic;

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
        trace!(
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
        trace!(
            "RPC request received at {}: {:?}",
            self.addr,
            request.get_ref()
        );

        let topic = &request.get_ref().topic;
        // Convert the message from Vec<u8> to Bytes
        let msg = Bytes::from(request.get_ref().msg.clone());

        match self
            .running_node
            .publish_on_topic(topic.clone(), msg.clone())
        {
            Ok(()) => Ok(Response::new(GossipsubPublishResponse {})),
            Err(err) => Err(Status::new(
                Code::Internal,
                format!("Failed to publish on topic '{topic}': {err}"),
            )),
        }
    }

    async fn stop(&self, request: Request<StopRequest>) -> Result<Response<StopResponse>, Status> {
        trace!(
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
        trace!(
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
        trace!(
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

pub(super) fn start_rpc_service(
    addr: SocketAddr,
    log_dir: &str,
    running_node: RunningNode,
    ctrl_tx: Sender<NodeCtrl>,
    started_instant: Instant,
) {
    // creating a service
    let service = SafeNodeRpcService {
        addr,
        log_dir: log_dir.to_string(),
        running_node,
        ctrl_tx,
        started_instant,
    };
    info!("RPC Server listening on {addr}");
    println!("RPC Server listening on {addr}");

    let _handle = tokio::spawn(async move {
        // adding our service to our server.
        Server::builder()
            .add_service(SafeNodeServer::new(service))
            .serve(addr)
            .await
    });
}
