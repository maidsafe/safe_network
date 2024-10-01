// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use autonomi::client::address::xorname_to_str;
use autonomi::Wallet;
use autonomi::Multiaddr;
use color_eyre::eyre::Context;
use color_eyre::eyre::Result;
use std::path::PathBuf;

pub async fn cost(file: &str, peers: Vec<Multiaddr>) -> Result<()> {
    let mut client = crate::actions::connect_to_network(peers).await?;

    println!("Getting upload cost...");
    let cost = client.file_cost(&PathBuf::from(file)).await
        .wrap_err("Failed to calculate cost for file")?;

    println!("Estimate cost to upload file: {file}");
    println!("Total cost: {cost}");
    Ok(())
}

pub async fn upload(file: &str, peers: Vec<Multiaddr>) -> Result<()> {
    let secret_key = crate::utils::get_secret_key()
        .wrap_err("The secret key is required to perform this action")?;
    let network = crate::utils::get_evm_network()
        .wrap_err("Failed to get evm network")?;
    let wallet = Wallet::new_from_private_key(network, &secret_key)
        .wrap_err("Failed to load wallet")?;

    let mut client = crate::actions::connect_to_network(peers).await?;

    println!("Uploading data to network...");
    let (_, xor_name) = client.upload_from_dir(PathBuf::from(file), &wallet).await
        .wrap_err("Failed to upload file")?;
    let addr = xorname_to_str(xor_name);

    println!("Successfully uploaded: {file}");
    println!("At address: {addr}");
    Ok(())
}

pub async fn download(addr: &str, dest_path: &str, peers: Vec<Multiaddr>) -> Result<()> {
    let mut client = crate::actions::connect_to_network(peers).await?;
    crate::actions::download(addr, dest_path, &mut client).await
}

pub fn list(_peers: Vec<Multiaddr>) -> Result<()> {
    println!("Listing previous uploads...");
    Ok(())
}
