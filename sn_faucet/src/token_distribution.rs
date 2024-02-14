// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::send_tokens;
use color_eyre::eyre::{eyre, Result};
use serde::{Deserialize, Serialize};
use sn_client::Client;
use sn_transfers::{MainPubkey, NanoTokens};
use std::str::FromStr;
use std::{collections::HashMap, path::PathBuf};
use tracing::info;
use url::Url;

const SNAPSHOT_FILENAME: &str = "snapshot.json";
const SNAPSHOT_URL: &str = "https://api.omniexplorer.info/ask.aspx?api=getpropertybalances&prop=3";
const PUBKEYS_URL: &str =
    "https://github.com/maidsafe/safe_network/raw/main/sn_faucet/maid_address_pubkeys.csv";
const HTTP_STATUS_OK: i32 = 200;

type MaidAddress = String; // base58 encoded
type MaidPubkey = String; // hex encoded
type Snapshot = HashMap<MaidAddress, NanoTokens>;

// Parsed from json in SNAPSHOT_URL
#[derive(Serialize, Deserialize)]
struct MaidBalance {
    address: MaidAddress,
    balance: String,
    reserved: String,
}

#[derive(Serialize, Deserialize)]
struct Distribution {
    #[serde(with = "serde_bytes")]
    transfer: Vec<u8>,
    #[serde(with = "serde_bytes")]
    encrypted_secret_key: Vec<u8>,
}

// This is different to test_faucet_data_dir because it should *not* be
// removed when --clean flag is specified.
fn get_snapshot_data_dir_path() -> Result<PathBuf> {
    let dir = dirs_next::data_dir()
        .ok_or_else(|| eyre!("could not obtain data directory path".to_string()))?
        .join("safe_snapshot");
    std::fs::create_dir_all(dir.clone())?;
    Ok(dir.to_path_buf())
}

fn get_pubkeys_data_dir_path() -> Result<PathBuf> {
    let dir = dirs_next::data_dir()
        .ok_or_else(|| eyre!("could not obtain data directory path".to_string()))?
        .join("safe_snapshot")
        .join("pubkeys");
    std::fs::create_dir_all(dir.clone())?;
    Ok(dir.to_path_buf())
}

fn get_distributions_data_dir_path() -> Result<PathBuf> {
    let dir = dirs_next::data_dir()
        .ok_or_else(|| eyre!("could not obtain data directory path".to_string()))?
        .join("safe_snapshot")
        .join("distributions");
    std::fs::create_dir_all(dir.clone())?;
    Ok(dir.to_path_buf())
}

pub fn load_maid_snapshot() -> Result<Snapshot> {
    // If the faucet restarts there will be an existing snapshot which should
    // be used to avoid conflicts in the balances between two different
    // snapshots.
    // Check if a previous snapshot already exists
    let root_dir = get_snapshot_data_dir_path()?;
    let filename = root_dir.join(SNAPSHOT_FILENAME);
    if std::fs::metadata(filename.clone()).is_ok() {
        info!("Using existing maid snapshot from {:?}", filename);
        maid_snapshot_from_file(filename)
    } else {
        info!("Fetching snapshot from {}", SNAPSHOT_URL);
        maid_snapshot_from_internet(filename)
    }
}

fn maid_snapshot_from_file(snapshot_path: PathBuf) -> Result<Snapshot> {
    let content = std::fs::read_to_string(snapshot_path)?;
    parse_snapshot(content)
}

fn maid_snapshot_from_internet(snapshot_path: PathBuf) -> Result<Snapshot> {
    // make the request
    let response = minreq::get(SNAPSHOT_URL).send()?;
    // check the request is ok
    if response.status_code != HTTP_STATUS_OK {
        let msg = format!("Snapshot failed with http status {}", response.status_code);
        return Err(eyre!(msg));
    }
    // write the response to file
    let body = response.as_str()?;
    info!("Writing snapshot to {:?}", snapshot_path);
    std::fs::write(snapshot_path.clone(), body)?;
    info!("Saved snapshot to {:?}", snapshot_path);
    // parse the json response
    parse_snapshot(body.to_string())
}

fn parse_snapshot(json_str: String) -> Result<Snapshot> {
    let balances: Vec<MaidBalance> = serde_json::from_str(&json_str)?;
    let mut balances_map: Snapshot = Snapshot::new();
    // verify the snapshot is ok
    // balances must match the ico amount, which is slightly higher than
    // 2^32/10 because of the ico process.
    // see https://omniexplorer.info/asset/3
    let supply = NanoTokens::from(452_552_412_000_000_000);
    let mut total = NanoTokens::zero();
    for b in &balances {
        // The reserved amount is the amount currently for sale on omni dex.
        // If it's not included the total is lower than expected.
        // So the amount of maid an address owns is balance + reserved.
        let balance = NanoTokens::from_str(&b.balance)?;
        let reserved = NanoTokens::from_str(&b.reserved)?;
        let address_balance = match balance.checked_add(reserved) {
            Some(b) => b,
            None => {
                let msg = format!("Nanos overflowed adding maid {balance} + {reserved}");
                return Err(eyre!(msg));
            }
        };
        total = match total.checked_add(address_balance) {
            Some(b) => b,
            None => {
                let msg = format!("Nanos overflowed adding maid {total} + {address_balance}");
                return Err(eyre!(msg));
            }
        };
        balances_map.insert(b.address.clone(), address_balance);
    }
    if total != supply {
        let msg = format!("Incorrect snapshot total, got {total} want {supply}");
        return Err(eyre!(msg));
    }
    // log the total number of balances that were parsed
    info!("Parsed {} maid balances from the snapshot", balances.len());
    Ok(balances_map)
}

fn load_maid_pubkeys_from_local() -> Result<HashMap<MaidAddress, MaidPubkey>> {
    let mut pubkeys = HashMap::new();
    // load from existing files
    let pk_dir = get_pubkeys_data_dir_path()?;
    let file_list = std::fs::read_dir(pk_dir)?;
    for file in file_list {
        // add to hashmap
        let file = file?;
        let pk_hex = std::fs::read_to_string(file.path())?;
        let address = match file.file_name().into_string() {
            Ok(s) => s,
            Err(s) => {
                let msg = format!("Error reading filename {s:?}");
                return Err(eyre!(msg));
            }
        };
        pubkeys.insert(address, pk_hex);
    }
    Ok(pubkeys)
}

pub fn load_maid_pubkeys() -> Result<HashMap<MaidAddress, MaidPubkey>> {
    info!("Loading public keys for distributions");
    let mut pubkeys = match load_maid_pubkeys_from_local() {
        Ok(pubkeys) => pubkeys,
        Err(err) => {
            info!("Failed to load pubkeys from local, {err:?}");
            HashMap::new()
        }
    };
    info!("{} pubkeys after reading existing files", pubkeys.len());

    // load from blockchain list on internet
    info!("Fetching pukeys from {PUBKEYS_URL}");
    let response = minreq::get(PUBKEYS_URL).send()?;
    // check the request is ok
    if response.status_code != 200 {
        println!(
            "Pubkey request failed with http status {}",
            response.status_code
        );
        // The existing data is ok, no need to fail to start the server here
        return Ok(pubkeys);
    }
    // parse the response as csv, each row has format:
    // address,pkhex
    let body = response.as_str()?;
    let lines: Vec<&str> = body.trim().split('\n').collect();
    info!("{} pubkey rows from {PUBKEYS_URL}", lines.len());
    for line in lines {
        let cells: Vec<&str> = line.split(',').collect();
        if cells.len() != 2 {
            continue;
        }
        let address = cells[0].trim().to_string();
        let pk_hex = cells[1].trim().to_string();
        // validate this pk corresponds to the address
        if !maid_pk_matches_address(&address, &pk_hex) {
            continue;
        }
        // save this pair to pk_dir
        save_address_pk(&address, &pk_hex)?;
        // add this pair to the hashmap
        pubkeys.insert(address, pk_hex);
    }
    info!(
        "{} pubkeys after reading from blockchain list",
        pubkeys.len()
    );
    Ok(pubkeys)
}

fn maid_pk_matches_address(address: &str, pk_hex: &str) -> bool {
    // parse the address
    let addr = match bitcoin::Address::from_str(address) {
        Ok(a) => a,
        Err(_) => return false,
    };
    let btc_addr = match addr.clone().require_network(bitcoin::Network::Bitcoin) {
        Ok(a) => a,
        Err(_) => return false,
    };
    // parse the public key
    let pk = match bitcoin::PublicKey::from_str(pk_hex) {
        Ok(p) => p,
        Err(_) => return false,
    };
    // The public key may be for a p2pkh address (starting with 1) or a p2wpkh
    // address (starting with 3) so we need to check both.
    let is_p2pkh = btc_addr.is_related_to_pubkey(&pk);
    if is_p2pkh {
        return true;
    }
    let p2wpkh_addr = match bitcoin::Address::p2shwpkh(&pk, bitcoin::Network::Bitcoin) {
        Ok(a) => a,
        Err(_) => return false,
    };
    let is_p2wpkh = p2wpkh_addr == addr;
    if is_p2wpkh {
        return true;
    }
    false
}

fn save_address_pk(address: &str, pk_hex: &str) -> Result<()> {
    let addr_path = get_pubkeys_data_dir_path()?.join(address);
    std::fs::write(addr_path, pk_hex)?;
    Ok(())
}

pub async fn distribute_from_maid_to_tokens(
    client: Client,
    snapshot: Snapshot,
    pubkeys: HashMap<MaidAddress, MaidPubkey>,
) {
    for (addr, amount) in snapshot {
        // check if this snapshot address has a pubkey
        if !pubkeys.contains_key(&addr) {
            continue;
        }
        let maid_pk = &pubkeys[&addr];
        let _ = create_distribution(&client, &addr, maid_pk, &amount).await;
    }
}

pub async fn handle_distribution_req(
    client: &Client,
    url: Url,
    balances: Snapshot,
) -> Result<String> {
    let query: HashMap<String, String> = url.query_pairs().into_owned().collect();
    let address = query
        .get("address")
        .ok_or(eyre!("Missing address in querystring"))?;
    let pkhex = query
        .get("pkhex")
        .ok_or(eyre!("Missing pkhex in querystring"))?;
    let amount = balances
        .get(address)
        .ok_or(eyre!("Address not in snapshot"))?;
    create_distribution(client, address, pkhex, amount).await
}

async fn create_distribution(
    client: &Client,
    addr: &MaidAddress,
    maid_pk: &MaidPubkey,
    amount: &NanoTokens,
) -> Result<String> {
    // validate the pk and the address match
    // because we can't be sure if this addr:pk pair has been pre-verified
    // and we don't want to encrypt using the wrong pubkey for the address
    if !maid_pk_matches_address(addr, maid_pk) {
        let msg = format!("Not creating distribution for mismatched addr/pk {addr} {maid_pk}");
        info!(msg);
        return Err(eyre!(msg));
    }
    // save this address and public key pair
    save_address_pk(addr, maid_pk)?;
    // check if this distribution has already been created
    let root = get_distributions_data_dir_path()?;
    let dist_path = root.join(addr);
    if dist_path.exists() {
        let dist_hex = match std::fs::read_to_string(dist_path.clone()) {
            Ok(content) => content,
            Err(err) => {
                let msg = format!(
                    "Error reading distribution file {}: {}",
                    dist_path.display(),
                    err
                );
                info!(msg);
                return Err(eyre!(msg));
            }
        };
        return Ok(dist_hex);
    }
    info!(
        "Distributing {} to {} using pubkey {}",
        amount, addr, maid_pk
    );
    // create a new random secret key to transfer this distribution to
    let dist_sk = bls::SecretKey::random();
    let dist_pk = MainPubkey(dist_sk.public_key()).to_hex();
    // create a transfer to this new distribution key
    let transfer_hex = match send_tokens(client, &amount.to_string(), &dist_pk).await {
        Ok(t) => t,
        Err(err) => {
            let msg = format!("Failed send for {addr}: {err}");
            info!(msg);
            return Err(eyre!(msg));
        }
    };
    let transfer = match hex::decode(transfer_hex) {
        Ok(t) => t,
        Err(err) => {
            let msg = format!("Failed to decode transfer to {addr}: {err}");
            info!(msg);
            return Err(eyre!(msg));
        }
    };
    // encrypt the secret key using the maid pubkey
    let dist_sk_bytes = dist_sk.to_bytes();
    let maid_pk_bytes = match hex::decode(maid_pk) {
        Ok(b) => b,
        Err(err) => {
            let msg = format!("Failed to decode maid pk {maid_pk}: {err}");
            info!(msg);
            return Err(eyre!(msg));
        }
    };
    let enc_dist_sk = match ecies::encrypt(&maid_pk_bytes, &dist_sk_bytes) {
        Ok(ct) => ct,
        Err(err) => {
            let msg = format!("Failed to encrypt secret key for {addr}: {err}");
            info!(msg);
            return Err(eyre!(msg));
        }
    };
    // create the distribution
    let dist = Distribution {
        transfer,
        encrypted_secret_key: enc_dist_sk,
    };
    // serialize the distribution using message pack
    let dist_bytes = match rmp_serde::to_vec_named(&dist) {
        Ok(b) => b,
        Err(err) => {
            let msg = format!("Failed to encode distribution for {addr}: {err}");
            info!(msg);
            return Err(eyre!(msg));
        }
    };
    let dist_hex = hex::encode(dist_bytes);
    // save the distribution
    match std::fs::write(dist_path.clone(), dist_hex.clone()) {
        Ok(_) => {}
        Err(err) => {
            let msg = format!(
                "Failed to write distribution to file {}: {}",
                dist_path.display(),
                err
            );
            info!(msg);
            info!("The distribution hex that failed to write to file:");
            info!(dist_hex);
            return Err(eyre!(msg));
        }
    };
    Ok(dist_hex)
}

#[cfg(all(test, feature = "distribution"))]
mod tests {
    use super::*;

    #[test]
    fn fetching_from_network() -> Result<()> {
        let snapshot = load_maid_snapshot()?;
        println!("Maid snapshot got {:?} entries", snapshot.len());
        assert_eq!(snapshot.len(), 16214);

        let pubkeys = load_maid_pubkeys()?;
        println!("Got {:?} distribution keys", pubkeys.len());
        assert_eq!(pubkeys.len(), 1227);

        let candidates = snapshot
            .iter()
            .filter(|(addr, _amount)| pubkeys.contains_key(*addr))
            .count();
        println!("Got {candidates:?} distribution candidates");
        assert_eq!(candidates, 1212);

        Ok(())
    }
}
