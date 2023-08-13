// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#![allow(clippy::mutable_key_type)]
mod common;

use assert_fs::TempDir;
use bytes::Bytes;
use common::{
    get_client_and_wallet, node_restart,
    safenode_proto::{safe_node_client::SafeNodeClient, NodeInfoRequest},
    DataLocationVerification, DATA_LOCATION_VERIFICATION_DELAY, PAYING_WALLET_INITIAL_BALANCE,
};
use eyre::Result;
use libp2p::{kad::RecordKey, PeerId};
use rand::{rngs::OsRng, Rng};
use sn_client::{Client, Files, WalletClient};
use sn_logging::{init_logging, LogFormat, LogOutputDest};
use sn_protocol::{storage::ChunkAddress, PrettyPrintRecordKey};
use sn_transfers::wallet::LocalWallet;
use std::{
    collections::{HashMap, HashSet},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};
use tonic::Request;
use tracing_core::Level;

const NODE_COUNT: u8 = 25;
const CHUNK_SIZE: usize = 1024;

// Default number of churns that should be performed. After each churn, we
// wait for VERIFICATION_DELAY time before verifying the data location.
// It can be overridden by setting the 'CHURN_COUNT' env var.
const CHURN_COUNT: u8 = 4;

/// Default number of chunks that should be PUT to the network.
// It can be overridden by setting the 'CHUNK_COUNT' env var.
const CHUNK_COUNT: usize = 5;

type NodeIndex = u8;
pub type RecordHolders = HashMap<RecordKey, HashSet<NodeIndex>>;

#[tokio::test(flavor = "multi_thread")]
async fn verify_data_location() -> Result<()> {
    let tmp_dir = std::env::temp_dir();
    let logging_targets = vec![
        ("safenode".to_string(), Level::TRACE),
        ("sn_transfers".to_string(), Level::TRACE),
        ("sn_networking".to_string(), Level::TRACE),
        ("sn_node".to_string(), Level::TRACE),
    ];
    let _log_appender_guard = init_logging(
        logging_targets,
        LogOutputDest::Path(tmp_dir.to_path_buf()),
        LogFormat::Default,
    )?;

    let churn_count = if let Ok(str) = std::env::var("CHURN_COUNT") {
        str.parse::<u8>()?
    } else {
        CHURN_COUNT
    };
    let chunk_count = if let Ok(str) = std::env::var("CHUNK_COUNT") {
        str.parse::<usize>()?
    } else {
        CHUNK_COUNT
    };
    println!(
        "Performing data location verification with a churn count of {churn_count} and n_chunks {chunk_count}\nIt will take approx {:?}",
        DATA_LOCATION_VERIFICATION_DELAY*churn_count as u32
    );

    // Store chunks
    println!("Creating a client and paying wallet...");
    let paying_wallet_dir = TempDir::new()?;

    let (client, paying_wallet) =
        get_client_and_wallet(paying_wallet_dir.path(), PAYING_WALLET_INITIAL_BALANCE).await?;

    store_chunk(client, paying_wallet, chunk_count).await?;

    let mut data_verification = DataLocationVerification::new(NODE_COUNT as usize).await?;

    // Verify data location initially
    data_verification.verify().await?;

    // Churn nodes and verify the location of the data after VERIFICATION_DELAY
    let mut addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 12000);
    let mut node_index = 1;
    let mut current_churn_count = 0;

    loop {
        if current_churn_count >= churn_count {
            break Ok(());
        }
        current_churn_count += 1;

        // restart a node
        addr.set_port(12000 + node_index);
        node_restart(addr).await?;

        // wait for the dead peer to be removed from the RT and the replication flow to finish
        println!("\nNode {node_index} has been restarted, waiting for {DATA_LOCATION_VERIFICATION_DELAY:?} before verification");
        tokio::time::sleep(DATA_LOCATION_VERIFICATION_DELAY).await;

        // get the new PeerId for the current NodeIndex
        let endpoint = format!("https://{addr}");
        let mut rpc_client = SafeNodeClient::connect(endpoint).await?;
        let response = rpc_client
            .node_info(Request::new(NodeInfoRequest {}))
            .await?;
        let peer_id = PeerId::from_bytes(&response.get_ref().peer_id)?;
        data_verification.update_peer_index(node_index as usize, peer_id)?;

        // get the new set of holders and verify
        data_verification.verify().await?;

        node_index += 1;
        if node_index > NODE_COUNT as u16 {
            node_index = 1;
        }
    }
}

// Generate a random Chunk and store it to the Network
async fn store_chunk(client: Client, paying_wallet: LocalWallet, chunk_count: usize) -> Result<()> {
    let mut rng = OsRng;
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);
    let file_api = Files::new(client);

    let mut uploaded_chunks_count = 0;
    loop {
        if uploaded_chunks_count >= chunk_count {
            break;
        }

        let random_bytes: Vec<u8> = ::std::iter::repeat(())
            .map(|()| rng.gen::<u8>())
            .take(CHUNK_SIZE)
            .collect();
        let bytes = Bytes::copy_from_slice(&random_bytes);

        let (addr, chunks) = file_api
            .chunk_bytes(bytes.clone())
            .expect("Failed to chunk bytes");

        println!(
            "Paying storage for ({}) new Chunk/s of file ({} bytes) at {addr:?}",
            chunks.len(),
            bytes.len()
        );

        let (proofs, _) = wallet_client
            .pay_for_storage(chunks.iter().map(|c| c.name()), true)
            .await
            .expect("Failed to pay for storage for new file at {addr:?}");

        println!(
            "Storing ({}) Chunk/s of file ({} bytes) at {addr:?}",
            chunks.len(),
            bytes.len()
        );

        let addr = ChunkAddress::new(file_api.calculate_address(bytes.clone())?);
        let key = PrettyPrintRecordKey::from(RecordKey::new(addr.xorname()));
        file_api.upload_with_proof(bytes, &proofs, true).await?;
        uploaded_chunks_count += 1;

        println!("Stored Chunk with {addr:?} / {key:?}");
    }

    // to make sure the last chunk was stored
    tokio::time::sleep(Duration::from_secs(10)).await;

    Ok(())
}
