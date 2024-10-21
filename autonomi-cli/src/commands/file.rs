// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::utils::collect_upload_summary;
use autonomi::client::address::addr_to_str;
use autonomi::client::address::str_to_addr;
use autonomi::Multiaddr;
use color_eyre::eyre::Context;
use color_eyre::eyre::Result;
use std::path::Path;
use std::path::PathBuf;

pub async fn cost(file: &str, peers: Vec<Multiaddr>) -> Result<()> {
    let client = crate::actions::connect_to_network(peers).await?;

    println!("Getting upload cost...");
    let cost = client
        .file_cost(&PathBuf::from(file))
        .await
        .wrap_err("Failed to calculate cost for file")?;

    println!("Estimate cost to upload file: {file}");
    println!("Total cost: {cost}");
    Ok(())
}
pub async fn upload(path: &str, peers: Vec<Multiaddr>) -> Result<()> {
    let wallet = crate::keys::load_evm_wallet()?;
    let mut client = crate::actions::connect_to_network(peers).await?;
    let event_receiver = client.enable_client_events();
    let (upload_summary_thread, upload_completed_tx) = collect_upload_summary(event_receiver);

    let path = PathBuf::from(path);

    let xor_name = if path.is_dir() {
        println!("Uploading directory: {path:?}");
        info!("Uploading directory: {path:?}");
        client
            .dir_upload(&path, &wallet)
            .await
            .wrap_err("Failed to upload directory")?
    } else {
        println!("Uploading file: {path:?}");
        info!("Uploading file: {path:?}");
        client
            .file_upload(&path, &wallet)
            .await
            .wrap_err("Failed to upload file")?
    };

    let addr = addr_to_str(xor_name);

    println!("Successfully uploaded: {path:?}");
    println!("At address: {addr}");
    info!("Successfully uploaded: {path:?} at address: {addr}");
    if let Ok(()) = upload_completed_tx.send(()) {
        let summary = upload_summary_thread.await?;
        if summary.record_count == 0 {
            println!("All chunks already exist on the network");
        } else {
            println!("Number of chunks uploaded: {}", summary.record_count);
            println!("Total cost: {} AttoTokens", summary.tokens_spent);
        }
        info!("Summary for upload of data {path:?} at {addr:?}: {summary:?}");
    }

    Ok(())
}
pub async fn download(addr: &str, dest_path: &Path, peers: Vec<Multiaddr>) -> Result<()> {
    let client = crate::actions::connect_to_network(peers).await?;
    let address = str_to_addr(addr).wrap_err("Failed to parse data address")?;

    client.download_file_or_dir(address, dest_path).await?;

    Ok(())
}

pub fn list(_peers: Vec<Multiaddr>) -> Result<()> {
    println!("The file list feature is coming soon!");
    Ok(())
}
