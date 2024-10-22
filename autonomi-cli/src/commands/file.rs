// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::utils::collect_upload_summary;
use autonomi::client::address::addr_to_str;
use autonomi::Multiaddr;
use color_eyre::eyre::Context;
use color_eyre::eyre::Result;
use color_eyre::Section;
use std::path::PathBuf;

pub async fn cost(file: &str, peers: Vec<Multiaddr>) -> Result<()> {
    let client = crate::actions::connect_to_network(peers).await?;

    println!("Getting upload cost...");
    info!("Calculating cost for file: {file}");
    let cost = client
        .file_cost(&PathBuf::from(file))
        .await
        .wrap_err("Failed to calculate cost for file")?;

    println!("Estimate cost to upload file: {file}");
    println!("Total cost: {cost}");
    info!("Total cost: {cost} for file: {file}");
    Ok(())
}

pub async fn upload(file: &str, peers: Vec<Multiaddr>) -> Result<()> {
    let wallet = crate::keys::load_evm_wallet()?;
    let mut client = crate::actions::connect_to_network(peers).await?;
    let event_receiver = client.enable_client_events();
    let (upload_summary_thread, upload_completed_tx) = collect_upload_summary(event_receiver);

    println!("Uploading data to network...");
    info!("Uploading file: {file}");

    let xor_name = client
        .dir_upload(PathBuf::from(file), &wallet)
        .await
        .wrap_err("Failed to upload file")?;
    let addr = addr_to_str(xor_name);

    if let Err(e) = upload_completed_tx.send(()) {
        error!("Failed to send upload completed event: {e:?}");
        eprintln!("Failed to send upload completed event: {e:?}");
    }

    let summary = upload_summary_thread.await?;
    if summary.record_count == 0 {
        println!("All chunks already exist on the network.");
    } else {
        println!("Successfully uploaded: {file}");
        println!("At address: {addr}");
        info!("Successfully uploaded: {file} at address: {addr}");
        println!("Number of chunks uploaded: {}", summary.record_count);
        println!("Total cost: {} AttoTokens", summary.tokens_spent);
    }
    info!("Summary for upload of file {file} at {addr:?}: {summary:?}");

    crate::user_data::write_local_file_archive(&xor_name, file)
        .wrap_err("Failed to save file to local user data")
        .with_suggestion(|| "Local user data saves the file address above to disk, without it you need to keep track of the address yourself")?;
    info!("Saved file to local user data");

    Ok(())
}

pub async fn download(addr: &str, dest_path: &str, peers: Vec<Multiaddr>) -> Result<()> {
    let mut client = crate::actions::connect_to_network(peers).await?;
    crate::actions::download(addr, dest_path, &mut client).await
}

pub fn list() -> Result<()> {
    println!("Retrieving local user data...");
    let file_archives = crate::user_data::get_local_file_archives()?;
    println!("✅ You have {} file archive(s):", file_archives.len());
    for (addr, name) in file_archives {
        println!("{}: {}", name, addr_to_str(addr));
    }
    Ok(())
}
