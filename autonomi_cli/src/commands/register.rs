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

pub fn cost(_name: &str, _peers: Vec<Multiaddr>) -> Result<()> {
    let _register_key = crate::utils::get_register_signing_key()
        .wrap_err("The register key is required to perform this action")?;
    println!("The register feature is coming soon!");
    Ok(())
}

pub fn create(_name: &str, _value: &str, _peers: Vec<Multiaddr>) -> Result<()> {
    let _secret_key = crate::utils::get_secret_key()
        .wrap_err("The secret key is required to perform this action")?;
    let _register_key = crate::utils::get_register_signing_key()
        .wrap_err("The register key is required to perform this action")?;
    println!("The register feature is coming soon!");
    Ok(())
}

pub fn edit(_name: &str, _value: &str, _peers: Vec<Multiaddr>) -> Result<()> {
    let _register_key = crate::utils::get_register_signing_key()
        .wrap_err("The register key is required to perform this action")?;
    println!("The register feature is coming soon!");
    Ok(())
}

pub fn get(_name: &str, _peers: Vec<Multiaddr>) -> Result<()> {
    let _register_key = crate::utils::get_register_signing_key()
        .wrap_err("The register key is required to perform this action")?;
    println!("The register feature is coming soon!");
    Ok(())
}

pub fn list(_peers: Vec<Multiaddr>) -> Result<()> {
    println!("The register feature is coming soon!");
    Ok(())
}
