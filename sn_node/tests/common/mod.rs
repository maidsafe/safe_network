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
use eyre::{eyre, Result};
use libp2p::PeerId;
use rand::{
    distributions::{Distribution, Standard},
    Rng,
};
use self_encryption::MIN_ENCRYPTABLE_BYTES;
use sn_client::{Client, Files};
use sn_protocol::safenode_proto::{
    safe_node_client::SafeNodeClient, NodeInfoRequest, RestartRequest,
};
use sn_protocol::storage::ChunkAddress;
use std::{
    fs::File,
    io::Write,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
};
use tonic::Request;
use xor_name::XorName;

type ResultRandomContent = Result<(Files, Bytes, ChunkAddress, Vec<(XorName, PathBuf)>)>;

pub fn random_content(
    client: &Client,
    wallet_dir: PathBuf,
    chunk_dir: PathBuf,
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

    let files_api = Files::new(client.clone(), wallet_dir);
    let (file_addr, _file_size, chunks) = Files::chunk_file(&file_path, &chunk_dir)?;

    Ok((
        files_api,
        random_length_content.into(),
        ChunkAddress::new(file_addr),
        chunks,
    ))
}

// Returns all the PeerId for all the locally running nodes
pub async fn get_all_peer_ids() -> Result<Vec<PeerId>> {
    let mut all_peers = Vec::new();

    let mut addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12000);
    for node_index in 1..NODE_COUNT + 1 {
        addr.set_port(12000 + node_index as u16);
        let endpoint = format!("https://{addr}");
        let mut rpc_client = SafeNodeClient::connect(endpoint).await?;

        // get the peer_id
        let response = rpc_client
            .node_info(Request::new(NodeInfoRequest {}))
            .await?;
        let peer_id = PeerId::from_bytes(&response.get_ref().peer_id)?;
        all_peers.push(peer_id);
    }
    println!("Obtained the PeerId list for the locally running network with a node count of {NODE_COUNT}");
    Ok(all_peers)
}

pub async fn node_restart(addr: SocketAddr) -> Result<()> {
    let endpoint = format!("https://{addr}");
    let mut client = SafeNodeClient::connect(endpoint).await?;

    let response = client.node_info(Request::new(NodeInfoRequest {})).await?;
    let log_dir = Path::new(&response.get_ref().log_dir);
    let root_dir = log_dir
        .parent()
        .ok_or_else(|| eyre!("could not obtain parent from logging directory"))?;

    // remove Chunks records
    let chunks_records = root_dir.join("record_store");
    if let Ok(true) = chunks_records.try_exists() {
        println!("Removing Chunks records from {}", chunks_records.display());
        std::fs::remove_dir_all(chunks_records)?;
    }

    // remove Registers records
    let registers_records = root_dir.join("registers");
    if let Ok(true) = registers_records.try_exists() {
        println!(
            "Removing Registers records from {}",
            registers_records.display()
        );
        std::fs::remove_dir_all(registers_records)?;
    }

    let _response = client
        .restart(Request::new(RestartRequest { delay_millis: 0 }))
        .await?;

    println!(
        "Node restart requested to RPC service at {addr}, and removed all its chunks and registers records at {}",
        log_dir.display()
    );

    Ok(())
}
