use color_eyre::eyre::bail;
use color_eyre::{eyre::eyre, Result};
use libp2p_identity::PeerId;
use sn_service_management::safenode_manager_proto::safe_node_manager_client::SafeNodeManagerClient;
use sn_service_management::safenode_manager_proto::NodeServiceRestartRequest;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;
use tonic::transport::Channel;
use tonic::Request;

struct DaemonRpcClient {
    addr: SocketAddr,
    rpc: SafeNodeManagerClient<Channel>,
}

pub async fn restart_node(
    peer_ids: Vec<String>,
    rpc_server_address: SocketAddr,
    retain_peer_id: bool,
) -> Result<()> {
    for peer_id in peer_ids {
        let str_bytes = PeerId::from_str(&peer_id)?.to_bytes();

        let mut daemon_client = get_rpc_client(rpc_server_address).await?;

        let _response = daemon_client
            .rpc
            .restart_node_service(Request::new(NodeServiceRestartRequest {
                peer_id: str_bytes,
                delay_millis: 0,
                retain_peer_id,
            }))
            .await
            .map_err(|err| {
                eyre!(
                    "Failed to restart node service with {peer_id:?} at {:?} with err: {err:?}",
                    daemon_client.addr
                )
            })?;
    }
    Ok(())
}

async fn get_rpc_client(socket_addr: SocketAddr) -> Result<DaemonRpcClient> {
    let endpoint = format!("https://{socket_addr}");
    let mut attempts = 0;
    loop {
        if let Ok(rpc_client) = SafeNodeManagerClient::connect(endpoint.clone()).await {
            let rpc_client = DaemonRpcClient {
                addr: socket_addr,
                rpc: rpc_client,
            };
            return Ok(rpc_client);
        }
        attempts += 1;
        println!("Could not connect to rpc {endpoint:?}. Attempts: {attempts:?}/10");
        tokio::time::sleep(Duration::from_secs(1)).await;
        if attempts >= 10 {
            bail!("Failed to connect to {endpoint:?} even after 10 retries");
        }
    }
}
