// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use safenode::node::{NodeEventsChannel, NodeId};

use super::NodeCtrl;

use eyre::{ErrReport, Result};
use std::{
    env,
    net::SocketAddr,
    time::{Duration, Instant},
};
use tokio::sync::mpsc::{self, Sender};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Code, Request, Response, Status};
use tracing::{debug, info, trace};

use safenode_proto::safe_node_server::{SafeNode, SafeNodeServer};
use safenode_proto::{
    NodeEvent, NodeEventsRequest, NodeInfoRequest, NodeInfoResponse, RestartRequest,
    RestartResponse, StopRequest, StopResponse, UpdateRequest, UpdateResponse,
};

// this includes code generated from .proto file
mod safenode_proto {
    tonic::include_proto!("safenode_proto");
}

// Defining a struct to hold information used by our gRPC service backend
struct SafeNodeRpcService {
    addr: SocketAddr,
    log_dir: String,
    node_events_chnnel: NodeEventsChannel,
    node_id: NodeId,
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
            node_id: self.node_id.as_bytes(),
            log_dir: self.log_dir.clone(),
            bin_version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_secs: self.started_instant.elapsed().as_secs(),
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

        let mut events_rx = self.node_events_chnnel.subscribe();
        let _handle = tokio::spawn(async move {
            while let Ok(event) = events_rx.recv().await {
                let event = NodeEvent {
                    event: format!("Event-{event:?}"),
                };

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
    log_dir: String,
    node_events_chnnel: NodeEventsChannel,
    node_id: NodeId,
    ctrl_tx: Sender<NodeCtrl>,
    started_instant: Instant,
) {
    // creating a service
    let service = SafeNodeRpcService {
        addr,
        log_dir,
        node_events_chnnel,
        node_id,
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
