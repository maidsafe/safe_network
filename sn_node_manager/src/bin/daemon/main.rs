// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[macro_use]
extern crate tracing;

use clap::Parser;
use color_eyre::eyre::{eyre, Result};
use libp2p_identity::PeerId;
use sn_node_manager::{config::get_node_registry_path, rpc, DAEMON_DEFAULT_PORT};
use sn_service_management::{
    safenode_manager_proto::{
        get_status_response::Node,
        safe_node_manager_server::{SafeNodeManager, SafeNodeManagerServer},
        GetStatusRequest, GetStatusResponse, NodeServiceRestartRequest, NodeServiceRestartResponse,
    },
    NodeRegistry,
};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tonic::{transport::Server, Code, Request, Response, Status};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Specify a port for the daemon to listen for RPCs. It defaults to 12500 if not set.
    #[clap(long, default_value_t = DAEMON_DEFAULT_PORT)]
    port: u16,
    /// Specify an Ipv4Addr for the daemon to listen on. This is useful if you want to manage the nodes remotely.
    ///
    /// If not set, the daemon listens locally for commands.
    #[clap(long, default_value_t = Ipv4Addr::new(127, 0, 0, 1))]
    address: Ipv4Addr,
}

struct SafeNodeManagerDaemon {}

// Implementing RPC interface for service defined in .proto
#[tonic::async_trait]
impl SafeNodeManager for SafeNodeManagerDaemon {
    async fn restart_node_service(
        &self,
        request: Request<NodeServiceRestartRequest>,
    ) -> Result<Response<NodeServiceRestartResponse>, Status> {
        println!("RPC request received {:?}", request.get_ref());
        info!("RPC request received {:?}", request.get_ref());
        let node_registry = Self::load_node_registry().map_err(|err| {
            Status::new(
                Code::Internal,
                format!("Failed to load node registry: {err}"),
            )
        })?;

        let peer_id = PeerId::from_bytes(&request.get_ref().peer_id)
            .map_err(|err| Status::new(Code::Internal, format!("Failed to parse PeerId: {err}")))?;

        Self::restart_handler(node_registry, peer_id, request.get_ref().retain_peer_id)
            .await
            .map_err(|err| {
                Status::new(Code::Internal, format!("Failed to restart the node: {err}"))
            })?;

        Ok(Response::new(NodeServiceRestartResponse {}))
    }

    async fn get_status(
        &self,
        request: Request<GetStatusRequest>,
    ) -> Result<Response<GetStatusResponse>, Status> {
        println!("RPC request received {:?}", request.get_ref());
        info!("RPC request received {:?}", request.get_ref());
        let node_registry = Self::load_node_registry().map_err(|err| {
            Status::new(
                Code::Internal,
                format!("Failed to load node registry: {err}"),
            )
        })?;

        let nodes_info = node_registry
            .nodes
            .iter()
            .map(|node| Node {
                peer_id: node.peer_id.map(|id| id.to_bytes()),
                status: node.status.clone() as i32,
                number: node.number as u32,
            })
            .collect::<Vec<_>>();
        Ok(Response::new(GetStatusResponse { nodes: nodes_info }))
    }
}

impl SafeNodeManagerDaemon {
    fn load_node_registry() -> Result<NodeRegistry> {
        let node_registry_path = get_node_registry_path()
            .map_err(|err| eyre!("Could not obtain node registry path: {err:?}"))?;
        let node_registry = NodeRegistry::load(&node_registry_path)
            .map_err(|err| eyre!("Could not load node registry: {err:?}"))?;
        Ok(node_registry)
    }

    async fn restart_handler(
        mut node_registry: NodeRegistry,
        peer_id: PeerId,
        retain_peer_id: bool,
    ) -> Result<()> {
        let res = rpc::restart_node_service(&mut node_registry, peer_id, retain_peer_id).await;

        // make sure to save the state even if the above fn fails.
        node_registry.save()?;

        res
    }
}

// The SafeNodeManager trait returns `Status` as its error. So the actual logic is here and we can easily map the errors
// into Status inside the trait fns.
impl SafeNodeManagerDaemon {}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    println!("Starting safenodemand");
    let args = Args::parse();
    let service = SafeNodeManagerDaemon {};

    // adding our service to our server.
    if let Err(err) = Server::builder()
        .add_service(SafeNodeManagerServer::new(service))
        .serve(SocketAddr::new(IpAddr::V4(args.address), args.port))
        .await
    {
        error!("Safenode Manager Daemon failed to start: {err:?}");
        println!("Safenode Manager Daemon failed to start: {err:?}");
        return Err(err.into());
    }

    Ok(())
}
