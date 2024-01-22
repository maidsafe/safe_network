// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{claim_genesis, send_tokens};
use color_eyre::eyre::{eyre, Result};
use serde::{Deserialize, Serialize};
use sn_client::Client;
use sn_transfers::{LocalWallet, NanoTokens};
use std::collections::HashMap;
use std::path::{self, Path, PathBuf};
use std::str::FromStr;
use tiny_http::{Response, Server};
use tracing::{debug, error, info, trace};

const SNAPSHOT_FILENAME: &str = "snapshot.json";
const SNAPSHOT_URL: &str = "https://api.omniexplorer.info/ask.aspx?api=getpropertybalances&prop=3";
const PUBKEYS_URL: &str = "https://pastebin.com/raw/pUm6tVRN";
const HTTP_STATUS_OK: i32 = 200;

type MaidAddress = String; // base58 encoded
type MaidPubkey = String; // hex encoded

// Parsed from json in SNAPSHOT_URL
#[derive(Serialize, Deserialize)]
struct MaidBalance {
    address: MaidAddress,
    balance: String,
    reserved: String,
}

/// Run the faucet server.
///
/// This will listen on port 8000 and send a transfer of tokens as response to any GET request.
///
/// # Example
///
/// ```bash
/// # run faucet server
/// cargo run  --features="local-discovery" --bin faucet --release -- server
///
/// # query faucet server for money for our address `get local wallet address`
/// curl "localhost:8000/`cargo run  --features="local-discovery"  --bin safe --release  wallet address | tail -n 1`" > transfer_hex
///
/// # receive transfer with our wallet
/// cargo run  --features="local-discovery" --bin safe --release  wallet receive --file transfer_hex
///
/// # balance should be updated
/// ```
pub async fn run_faucet_server(client: &Client) -> Result<()> {
    claim_genesis(client).await.map_err(|err| {
        println!("Faucet Server couldn't start as we failed to claim Genesis");
        eprintln!("Faucet Server couldn't start as we failed to claim Genesis");
        error!("Faucet Server couldn't start as we failed to claim Genesis");
        err
    })?;
    startup_server(client).await
}

pub async fn restart_faucet_server(client: &Client) -> Result<()> {
    let root_dir = get_test_faucet_data_dir_path()?;
    println!("Loading the previous wallet at {root_dir:?}");
    debug!("Loading the previous wallet at {root_dir:?}");

    deposit(&root_dir)?;

    println!("Previous wallet loaded");
    debug!("Previous wallet loaded");

    startup_server(client).await
}

async fn startup_server(client: &Client) -> Result<()> {
    let _balances = load_maid_snapshot()?;
    let _keys = load_maid_pubkeys()?;
    let server =
        Server::http("0.0.0.0:8000").map_err(|err| eyre!("Failed to start server: {err}"))?;

    // This println is used in sn_testnet to wait for the faucet to start.
    println!("Starting http server listening on port 8000...");
    debug!("Starting http server listening on port 8000...");
    for request in server.incoming_requests() {
        println!(
            "received request! method: {:?}, url: {:?}, headers: {:?}",
            request.method(),
            request.url(),
            request.headers()
        );
        trace!(
            "received request! method: {:?}, url: {:?}, headers: {:?}",
            request.method(),
            request.url(),
            request.headers()
        );
        let key = request.url().trim_matches(path::is_separator);

        match send_tokens(client, "100", key).await {
            Ok(transfer) => {
                println!("Sent tokens to {key}");
                debug!("Sent tokens to {key}");
                let response = Response::from_string(transfer);
                let _ = request.respond(response).map_err(|err| {
                    eprintln!("Failed to send response: {err}");
                    error!("Failed to send response: {err}");
                });
            }
            Err(err) => {
                eprintln!("Failed to send tokens to {key}: {err}");
                error!("Failed to send tokens to {key}: {err}");
                let response = Response::from_string(format!("Failed to send tokens: {err}"));
                let _ = request
                    .respond(response.with_status_code(500))
                    .map_err(|err| eprintln!("Failed to send response: {err}"));
            }
        }
    }
    Ok(())
}

fn get_test_faucet_data_dir_path() -> Result<PathBuf> {
    let home_dirs = dirs_next::data_dir()
        .ok_or_else(|| eyre!("could not obtain data directory path".to_string()))?
        .join("safe")
        .join("test_faucet");
    std::fs::create_dir_all(home_dirs.clone())?;
    Ok(home_dirs.to_path_buf())
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

fn deposit(root_dir: &Path) -> Result<()> {
    let mut wallet = LocalWallet::load_from(root_dir)?;

    let previous_balance = wallet.balance();

    wallet.try_load_cash_notes()?;

    let deposited = NanoTokens::from(wallet.balance().as_nano() - previous_balance.as_nano());
    if deposited.is_zero() {
        println!("Nothing deposited.");
    } else if let Err(err) = wallet.deposit_and_store_to_disk(&vec![]) {
        println!("Failed to store deposited ({deposited}) amount: {err:?}");
    } else {
        println!("Deposited {deposited}.");
    }

    Ok(())
}

fn load_maid_snapshot() -> Result<HashMap<MaidAddress, NanoTokens>> {
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

fn maid_snapshot_from_file(snapshot_path: PathBuf) -> Result<HashMap<MaidAddress, NanoTokens>> {
    let content = std::fs::read_to_string(snapshot_path)?;
    parse_snapshot(content)
}

fn maid_snapshot_from_internet(snapshot_path: PathBuf) -> Result<HashMap<MaidAddress, NanoTokens>> {
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

fn parse_snapshot(json_str: String) -> Result<HashMap<MaidAddress, NanoTokens>> {
    let balances: Vec<MaidBalance> = serde_json::from_str(&json_str)?;
    let mut balances_map: HashMap<MaidAddress, NanoTokens> = HashMap::new();
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

fn load_maid_pubkeys() -> Result<HashMap<MaidAddress, MaidPubkey>> {
    info!("Loading public keys for distributions");
    let mut pubkeys: HashMap<MaidAddress, MaidPubkey> = HashMap::new();
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
    info!("{} pubkeys after reading existing files", pubkeys.len());
    // load from blockchain list on internet
    info!("Fetching pukeys from {PUBKEYS_URL}");
    let response = minreq::get(PUBKEYS_URL).send()?;
    // check the request is ok
    if response.status_code != 200 {
        let msg = format!(
            "Pubkey request failed with http status {}",
            response.status_code
        );
        return Err(eyre!(msg));
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
