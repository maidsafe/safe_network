// Copyright (C) 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[macro_use]
extern crate tracing;

use color_eyre::{self, eyre::Result};
use sn_protocol::safenode_manager_proto::{
    safe_node_manager_server::{SafeNodeManager, SafeNodeManagerServer},
    RestartRequest, RestartResponse,
};
use std::net::{Ipv4Addr, SocketAddr};
use tonic::{transport::Server, Request, Response, Status};

struct SafeNodeManagerDaemon {}

// Implementing RPC interface for service defined in .proto
#[tonic::async_trait]
impl SafeNodeManager for SafeNodeManagerDaemon {
    async fn restart(
        &self,
        request: Request<RestartRequest>,
    ) -> Result<Response<RestartResponse>, Status> {
        println!("RPC request received {:?}", request.get_ref());

        // let delay = Duration::from_millis(request.get_ref().delay_millis);
        // match self.ctrl_tx.send(NodeCtrl::Restart(delay)).await {
        //     Ok(()) => Ok(Response::new(RestartResponse {})),
        //     Err(err) => Err(Status::new(
        //         Code::Internal,
        //         format!("Failed to restart the node: {err}"),
        //     )),
        // }
        Ok(Response::new(RestartResponse {}))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let service = SafeNodeManagerDaemon {};

    // adding our service to our server.
    if let Err(err) = Server::builder()
        .add_service(SafeNodeManagerServer::new(service))
        .serve(SocketAddr::new(
            std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            12989,
        ))
        .await
    {
        error!("Safenode Manager Daemon failed to start: {err:?}");
        println!("Safenode Manager Daemon failed to start: {err:?}");
        return Err(err.into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use color_eyre::eyre::{bail, Result};
    use sn_protocol::safenode_manager_proto::{
        safe_node_manager_client::SafeNodeManagerClient, RestartRequest,
    };
    use std::{
        net::{Ipv4Addr, SocketAddr},
        time::Duration,
    };
    use tonic::Request;

    #[tokio::test]
    async fn restart() -> Result<()> {
        let mut rpc_client = get_safenode_manager_rpc_client(SocketAddr::new(
            std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            12989,
        ))
        .await?;

        let response = rpc_client
            .restart(Request::new(RestartRequest { delay_millis: 0 }))
            .await?;
        println!("response: {response:?}");

        Ok(())
    }

    // Connect to a RPC socket addr with retry
    pub async fn get_safenode_manager_rpc_client(
        socket_addr: SocketAddr,
    ) -> Result<SafeNodeManagerClient<tonic::transport::Channel>> {
        // get the new PeerId for the current NodeIndex
        let endpoint = format!("https://{socket_addr}");
        let mut attempts = 0;
        loop {
            if let Ok(rpc_client) = SafeNodeManagerClient::connect(endpoint.clone()).await {
                break Ok(rpc_client);
            }
            attempts += 1;
            println!("Could not connect to rpc {endpoint:?}. Attempts: {attempts:?}/10");
            error!("Could not connect to rpc {endpoint:?}. Attempts: {attempts:?}/10");
            tokio::time::sleep(Duration::from_secs(1)).await;
            if attempts >= 10 {
                bail!("Failed to connect to {endpoint:?} even after 10 retries");
            }
        }
    }
}
