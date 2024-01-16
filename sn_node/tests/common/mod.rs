// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.
#![allow(dead_code)]

pub mod client;

use bytes::Bytes;
use eyre::{bail, Result};
use libp2p::PeerId;
use rand::{
    distributions::{Distribution, Standard},
    Rng,
};
use self_encryption::MIN_ENCRYPTABLE_BYTES;
use sn_client::{Client, FilesApi};
use sn_protocol::safenode_proto::{
    safe_node_client::SafeNodeClient, NodeInfoRequest, RestartRequest,
};
use sn_protocol::storage::ChunkAddress;
use std::{
    fs::File,
    io::Write,
    net::SocketAddr,
    path::{Path, PathBuf},
    time::Duration,
};
use tonic::Request;
use tracing::{debug, error, info};
use xor_name::XorName;

type ResultRandomContent = Result<(FilesApi, Bytes, ChunkAddress, Vec<(XorName, PathBuf)>)>;

pub fn random_content(
    client: &Client,
    wallet_dir: PathBuf,
    chunk_dir: &Path,
) -> ResultRandomContent {
    let mut rng = rand::thread_rng();

    let random_len = rng.gen_range(MIN_ENCRYPTABLE_BYTES..1024 * MIN_ENCRYPTABLE_BYTES);
    let random_length_content: Vec<u8> =
        <Standard as Distribution<u8>>::sample_iter(Standard, &mut rng)
            .take(random_len)
            .collect();

    let file_path = chunk_dir.join("random_content");
    let mut output_file = File::create(file_path.clone())?;
    output_file.write_all(&random_length_content)?;

    let files_api = FilesApi::new(client.clone(), wallet_dir);
    let (head_chunk_address, _data_map, _file_size, chunks) =
        FilesApi::chunk_file(&file_path, chunk_dir, true)?;

    Ok((
        files_api,
        random_length_content.into(),
        head_chunk_address,
        chunks,
    ))
}

// Connect to a RPC socket addr with retry
pub async fn get_safenode_rpc_client(
    socket_addr: SocketAddr,
) -> Result<SafeNodeClient<tonic::transport::Channel>> {
    // get the new PeerId for the current NodeIndex
    let endpoint = format!("https://{socket_addr}");
    let mut attempts = 0;
    loop {
        if let Ok(rpc_client) = SafeNodeClient::connect(endpoint.clone()).await {
            break Ok(rpc_client);
        }
        println!("Could not connect to rpc {endpoint:?} after restarting. retrying");
        error!("Could not connect to rpc {endpoint:?} after restarting. retrying");
        tokio::time::sleep(Duration::from_secs(1)).await;
        attempts += 1;
        if attempts >= 10 {
            bail!("FAILED TO CONNECT to {endpoint:?} even after 10 retries");
        }
    }
}

// Returns all the PeerId for all the running nodes
pub async fn get_all_peer_ids(node_rpc_addresses: &Vec<SocketAddr>) -> Result<Vec<PeerId>> {
    let mut all_peers = Vec::new();

    for addr in node_rpc_addresses {
        let mut rpc_client = get_safenode_rpc_client(*addr).await?;

        // get the peer_id
        let response = rpc_client
            .node_info(Request::new(NodeInfoRequest {}))
            .await?;
        let peer_id = PeerId::from_bytes(&response.get_ref().peer_id)?;
        all_peers.push(peer_id);
    }
    debug!(
        "Obtained the PeerId list for the running network with a node count of {}",
        node_rpc_addresses.len()
    );
    Ok(all_peers)
}

pub async fn node_restart(addr: &SocketAddr) -> Result<()> {
    let mut rpc_client = get_safenode_rpc_client(*addr).await?;

    let _response = rpc_client
        .restart(Request::new(RestartRequest { delay_millis: 0 }))
        .await?;

    println!("Node restart requested to RPC service at {addr}");
    info!("Node restart requested to RPC service at {addr}");

    Ok(())
}
