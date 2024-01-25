// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use color_eyre::{eyre::eyre, Result};
use sn_client::Client;
use sn_transfers::{LocalWallet, SpendAddress, Transfer, UniquePubkey, GENESIS_CASHNOTE};
use std::path::Path;
use url::Url;

pub async fn audit(
    client: &Client,
    to_dot: bool,
    find_royalties: bool,
    root_dir: &Path,
) -> Result<()> {
    let genesis_addr = SpendAddress::from_unique_pubkey(&GENESIS_CASHNOTE.unique_pubkey());

    if to_dot {
        let dag = client.build_spend_dag_from(genesis_addr).await?;
        println!("{}", dag.dump_dot_format());
    } else {
        println!("Auditing the Currency, note that this might take a very long time...");
        client
            .follow_spend(genesis_addr, find_royalties, root_dir)
            .await?;
    }

    Ok(())
}

pub async fn get_faucet(root_dir: &Path, client: &Client, url: String) -> Result<()> {
    let wallet = LocalWallet::load_from(root_dir)?;
    let address_hex = wallet.address().to_hex();
    let url = if !url.contains("://") {
        format!("{}://{}", "http", url)
    } else {
        url
    };
    let req_url = Url::parse(&format!("{url}/{address_hex}"))?;
    println!("Requesting token for wallet address: {address_hex}");

    let response = reqwest::get(req_url).await?;
    let is_ok = response.status().is_success();
    let body = response.text().await?;
    if is_ok {
        receive(body, false, client, root_dir).await?;
        println!("Successfully got tokens from faucet.");
    } else {
        println!("Failed to get tokens from faucet, server responded with: {body:?}");
    }
    Ok(())
}

pub async fn receive(
    transfer: String,
    is_file: bool,
    client: &Client,
    root_dir: &Path,
) -> Result<()> {
    let transfer = if is_file {
        std::fs::read_to_string(transfer)?.trim().to_string()
    } else {
        transfer
    };

    let transfer = match Transfer::from_hex(&transfer) {
        Ok(transfer) => transfer,
        Err(err) => {
            println!("Failed to parse transfer: {err:?}");
            println!("Transfer: \"{transfer}\"");
            return Err(err.into());
        }
    };
    println!("Successfully parsed transfer. ");

    println!("Verifying transfer with the Network...");
    let mut wallet = LocalWallet::load_from(root_dir)?;
    let cashnotes = match client.receive(&transfer, &wallet).await {
        Ok(cashnotes) => cashnotes,
        Err(err) => {
            println!("Failed to verify and redeem transfer: {err:?}");
            return Err(err.into());
        }
    };
    println!("Successfully verified transfer.");

    let old_balance = wallet.balance();
    wallet.deposit_and_store_to_disk(&cashnotes)?;
    let new_balance = wallet.balance();

    println!("Successfully stored cash_note to wallet dir.");
    println!("Old balance: {old_balance}");
    println!("New balance: {new_balance}");

    Ok(())
}

/// Verify a spend on the Network.
/// if genesis is true, verify all the way to Genesis, note that this might take A VERY LONG TIME
pub async fn verify_spend(spend_address: String, genesis: bool, client: &Client) -> Result<()> {
    if genesis {
        println!("Verifying spend all the way to Genesis, note that this might take a while...");
    } else {
        println!("Verifying spend...");
    }

    let addr = parse_pubkey_address(&spend_address)?;
    match client.verify_spend(addr, genesis).await {
        Ok(()) => println!("Spend verified to be stored and unique at {addr:?}"),
        Err(e) => println!("Failed to verify spend at {addr:?}: {e}"),
    }

    Ok(())
}

pub fn parse_pubkey_address(str_addr: &str) -> Result<SpendAddress> {
    let pk_res = UniquePubkey::from_hex(str_addr);
    let addr_res = SpendAddress::from_hex(str_addr);

    match (pk_res, addr_res) {
        (Ok(pk), _) => Ok(SpendAddress::from_unique_pubkey(&pk)),
        (_, Ok(addr)) => Ok(addr),
        _ => Err(eyre!("Failed to parse address: {str_addr}")),
    }
}
