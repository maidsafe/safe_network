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

#[expect(clippy::unused_async)]
pub async fn cost(name: &str, _peers: Vec<Multiaddr>) -> Result<()> {
    let register_key = crate::utils::get_register_signing_key()
        .wrap_err("The register key is required to perform this action")?;
    println!("Estimate cost to register name: {name} with register key: {register_key}");
    Ok(())
}

#[expect(clippy::unused_async)]
pub async fn create(name: &str, value: &str, _peers: Vec<Multiaddr>) -> Result<()> {
    let secret_key = crate::utils::get_secret_key()
        .wrap_err("The secret key is required to perform this action")?;
    let register_key = crate::utils::get_register_signing_key()
        .wrap_err("The register key is required to perform this action")?;
    println!(
        "Creating register: {name} with value: {value} using secret key: {secret_key} and register key: {register_key}"
    );
    Ok(())
}

#[expect(clippy::unused_async)]
pub async fn edit(name: &str, value: &str, _peers: Vec<Multiaddr>) -> Result<()> {
    let register_key = crate::utils::get_register_signing_key()
        .wrap_err("The register key is required to perform this action")?;
    println!("Editing register: {name} with value: {value} using register key: {register_key}");
    Ok(())
}

#[expect(clippy::unused_async)]
pub async fn get(name: &str, _peers: Vec<Multiaddr>) -> Result<()> {
    let register_key = crate::utils::get_register_signing_key()
        .wrap_err("The register key is required to perform this action")?;
    println!("Getting value of register: {name} with register key: {register_key}");
    Ok(())
}

pub fn list(_peers: Vec<Multiaddr>) -> Result<()> {
    println!("Listing previous registers...");
    Ok(())
}
