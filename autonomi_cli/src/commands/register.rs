// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use autonomi::client::registers::RegisterAddress;
use autonomi::client::registers::RegisterSecretKey;
use autonomi::Multiaddr;
use color_eyre::eyre::eyre;
use color_eyre::eyre::Context;
use color_eyre::eyre::Result;
use color_eyre::Section;

pub fn generate_key(overwrite: bool) -> Result<()> {
    // check if the key already exists
    let key_path = crate::keys::get_register_signing_key_path()?;
    if key_path.exists() && !overwrite {
        return Err(eyre!("Register key already exists at: {}", key_path.display()))
            .with_suggestion(|| "if you want to overwrite the existing key, run the command with the --overwrite flag")
            .with_warning(|| "overwriting the existing key might result in loss of access to any existing registers created using that key");
    }

    // generate and write a new key to file
    let key = RegisterSecretKey::random();
    let path = crate::keys::create_register_signing_key_file(key)
        .wrap_err("Failed to create new register key")?;
    println!("✅ Created new register key at: {}", path.display());
    Ok(())
}

pub async fn cost(name: &str, peers: Vec<Multiaddr>) -> Result<()> {
    let register_key = crate::keys::get_register_signing_key()
        .wrap_err("The register key is required to perform this action")?;
    let client = crate::actions::connect_to_network(peers).await?;

    let cost = client
        .register_cost(name.to_string(), register_key)
        .await
        .wrap_err("Failed to get cost for register")?;
    println!("✅ The estimated cost to create a register with name {name} is: {cost}");
    Ok(())
}

pub async fn create(name: &str, value: &str, peers: Vec<Multiaddr>) -> Result<()> {
    let wallet = crate::keys::load_evm_wallet()?;
    let register_key = crate::keys::get_register_signing_key()
        .wrap_err("The register key is required to perform this action")?;
    let client = crate::actions::connect_to_network(peers).await?;

    println!("Creating register with name: {name}");
    let register = client
        .register_create(
            value.as_bytes().to_vec().into(),
            name,
            register_key,
            &wallet,
        )
        .await
        .wrap_err("Failed to create register")?;
    let address = register.address();

    println!("✅ Register created at address: {address}");
    println!("With name: {name}");
    println!("And initial value: [{value}]");
    Ok(())
}

pub async fn edit(address: String, name: bool, value: &str, peers: Vec<Multiaddr>) -> Result<()> {
    let register_key = crate::keys::get_register_signing_key()
        .wrap_err("The register key is required to perform this action")?;
    let client = crate::actions::connect_to_network(peers).await?;

    let address = if name {
        client.register_address(&address, &register_key)
    } else {
        RegisterAddress::from_hex(&address)
            .wrap_err(format!("Failed to parse register address: {address}"))
            .with_suggestion(|| "if you want to use the name as the address, run the command with the --name flag")?
    };

    println!("Getting register at address: {address}");
    let register = client
        .register_get(address)
        .await
        .wrap_err(format!("Failed to get register at address: {address}"))?;
    println!("Found register at address: {address}");

    println!("Updating register with new value: {value}");
    client
        .register_update(register, value.as_bytes().to_vec().into(), register_key)
        .await
        .wrap_err(format!("Failed to update register at address: {address}"))?;

    println!("✅ Successfully updated register");
    println!("With value: [{value}]");

    Ok(())
}

pub async fn get(address: String, name: bool, peers: Vec<Multiaddr>) -> Result<()> {
    let register_key = crate::keys::get_register_signing_key()
        .wrap_err("The register key is required to perform this action")?;
    let client = crate::actions::connect_to_network(peers).await?;

    let address = if name {
        client.register_address(&address, &register_key)
    } else {
        RegisterAddress::from_hex(&address)
            .wrap_err(format!("Failed to parse register address: {address}"))
            .with_suggestion(|| "if you want to use the name as the address, run the command with the --name flag")?
    };

    println!("Getting register at address: {address}");
    let register = client
        .register_get(address)
        .await
        .wrap_err(format!("Failed to get register at address: {address}"))?;
    let values = register.values();

    println!("✅ Register found at address: {address}");
    match values.as_slice() {
        [one] => println!("With value: [{:?}]", String::from_utf8_lossy(one)),
        _ => {
            println!("With multiple concurrent values:");
            for value in values.iter() {
                println!("[{:?}]", String::from_utf8_lossy(value));
            }
        }
    }
    Ok(())
}

pub fn list(_peers: Vec<Multiaddr>) -> Result<()> {
    println!("The register feature is coming soon!");
    Ok(())
}
