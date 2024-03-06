// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[cfg(feature = "distribution")]
use super::WalletApiHelper;
#[cfg(feature = "distribution")]
use base64::Engine;
use color_eyre::Result;
use sn_client::transfers::{HotWallet, SpendAddress, Transfer};
use sn_client::Client;
use std::{path::Path, str::FromStr};
use url::Url;

#[cfg(feature = "distribution")]
pub async fn get_faucet(
    root_dir: &Path,
    client: &Client,
    url: String,
    address: Option<String>,
    signature: Option<String>,
) -> Result<()> {
    if address.is_some() ^ signature.is_some() {
        println!("Address and signature must both be specified.");
        return Ok(());
    }
    if address.is_none() && signature.is_none() {
        get_faucet_fixed_amount(root_dir, client, url).await?;
    } else if let Some(addr) = address {
        if let Some(sig) = signature {
            get_faucet_distribution(root_dir, client, url, addr, sig).await?;
        }
    }
    Ok(())
}

#[cfg(not(feature = "distribution"))]
pub async fn get_faucet(
    root_dir: &Path,
    client: &Client,
    url: String,
    _address: Option<String>,
    _signature: Option<String>,
) -> Result<()> {
    get_faucet_fixed_amount(root_dir, client, url).await
}

pub async fn get_faucet_fixed_amount(root_dir: &Path, client: &Client, url: String) -> Result<()> {
    let wallet = HotWallet::load_from(root_dir)?;
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

#[cfg(feature = "distribution")]
pub async fn get_faucet_distribution(
    root_dir: &Path,
    client: &Client,
    url: String,
    address: String,
    signature: String,
) -> Result<()> {
    // submit the details to the faucet to get the distribution
    let url = if !url.contains("://") {
        format!("{}://{}", "http", url)
    } else {
        url
    };
    // receive to the current local wallet
    let wallet = WalletApiHelper::load_from(root_dir)?.address().to_hex();
    println!("Requesting distribution for maid address {address} to local wallet {wallet}");
    // base64 uses + and / as the delimiters which doesn't go well in the query
    // string, so the signature is encoded using url safe characters.
    let sig_bytes = base64::engine::general_purpose::STANDARD.decode(signature)?;
    let sig_url = base64::engine::general_purpose::URL_SAFE.encode(sig_bytes);
    let req_url = Url::parse(&format!(
        "{url}/distribution?address={address}&wallet={wallet}&signature={sig_url}"
    ))?;
    let response = reqwest::get(req_url).await?;
    let is_ok = response.status().is_success();
    let transfer_hex = response.text().await?;
    if !is_ok {
        println!(
            "Failed to get distribution from faucet, server responded with:\n{transfer_hex:?}"
        );
        return Ok(());
    }
    println!("Receiving transfer for maid address {address}:\n{transfer_hex}");
    receive(transfer_hex, false, client, root_dir).await?;
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
    let mut wallet = HotWallet::load_from(root_dir)?;
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
pub async fn verify_spend_at(spend_address: String, genesis: bool, client: &Client) -> Result<()> {
    if genesis {
        println!("Verifying spend all the way to Genesis, note that this might take a while...");
    } else {
        println!("Verifying spend...");
    }

    let addr = SpendAddress::from_str(&spend_address)?;
    match client.verify_spend_at(addr, genesis).await {
        Ok(()) => println!("Spend verified to be stored and unique at {addr:?}"),
        Err(e) => println!("Failed to verify spend at {addr:?}: {e}"),
    }

    Ok(())
}
