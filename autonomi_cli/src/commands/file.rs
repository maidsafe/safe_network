// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use autonomi::Multiaddr;
use color_eyre::eyre::Context;
use color_eyre::eyre::Result;

pub fn cost(file: &str, peers: Vec<Multiaddr>) -> Result<()> {
    println!("Estimate cost to upload file: {file}");
    Ok(())
}

pub fn upload(file: &str, peers: Vec<Multiaddr>) -> Result<()> {
    let secret_key = crate::utils::get_secret_key()
        .wrap_err("The secret key is required to perform this action")?;
    println!("Uploading file: {file} with secret key: {secret_key}");
    Ok(())
}

pub fn download(addr: &str, dest_file: &str, peers: Vec<Multiaddr>) -> Result<()> {
    println!("Downloading file from {addr} to {dest_file}");
    Ok(())
}

pub fn list(peers: Vec<Multiaddr>) -> Result<()> {
    println!("Listing previous uploads...");
    Ok(())
}
