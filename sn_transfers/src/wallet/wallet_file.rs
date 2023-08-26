// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::client_transfers::SpendRequest;

use super::{
    error::{Error, Result},
    KeyLessWallet,
};

use sn_dbc::{Dbc, DbcId};
use sn_protocol::storage::DbcAddress;
use std::path::{Path, PathBuf};
use tokio::fs;

// Filename for storing a wallet.
const WALLET_FILE_NAME: &str = "wallet";
const CREATED_DBCS_DIR_NAME: &str = "created_dbcs";
const RECEIVED_DBCS_DIR_NAME: &str = "received_dbcs";
const UNCONFRIMED_TX_NAME: &str = "unconfirmed_txs";

pub(super) async fn create_received_dbcs_dir(wallet_dir: &Path) -> Result<()> {
    let received_dbcs_dir = wallet_dir.join(RECEIVED_DBCS_DIR_NAME);
    fs::create_dir_all(&received_dbcs_dir).await?;
    Ok(())
}
/// Writes the `KeyLessWallet` to the specified path.
pub(super) async fn store_wallet(wallet_dir: &Path, wallet: &KeyLessWallet) -> Result<()> {
    let wallet_path = wallet_dir.join(WALLET_FILE_NAME);
    let bytes = bincode::serialize(&wallet)?;
    fs::write(&wallet_path, bytes).await?;
    Ok(())
}

/// Returns `Some(KeyLessWallet)` or None if file doesn't exist.
pub(super) async fn get_wallet(wallet_dir: &Path) -> Result<Option<KeyLessWallet>> {
    let path = wallet_dir.join(WALLET_FILE_NAME);
    if !path.is_file() {
        return Ok(None);
    }

    let bytes = fs::read(&path).await?;
    let wallet = bincode::deserialize(&bytes)?;

    Ok(Some(wallet))
}

/// Writes the `unconfirmed_txs` to the specified path.
pub(super) async fn store_unconfirmed_txs(
    wallet_dir: &Path,
    unconfirmed_txs: &Vec<SpendRequest>,
) -> Result<()> {
    let unconfirmed_txs_path = wallet_dir.join(UNCONFRIMED_TX_NAME);
    let bytes = bincode::serialize(&unconfirmed_txs)?;
    fs::write(&unconfirmed_txs_path, bytes).await?;
    Ok(())
}

/// Returns `Some(Vec<SpendRequest>)` or None if file doesn't exist.
pub(super) async fn get_unconfirmed_txs(wallet_dir: &Path) -> Result<Option<Vec<SpendRequest>>> {
    let path = wallet_dir.join(UNCONFRIMED_TX_NAME);
    if !path.is_file() {
        return Ok(None);
    }

    let bytes = fs::read(&path).await?;
    let unconfirmed_txs = bincode::deserialize(&bytes)?;

    Ok(Some(unconfirmed_txs))
}

/// Hex encode and write each `Dbc` to a separate file in respective
/// recipient public address dir in the created dbcs dir. Each file is named after the dbc id.
pub(super) async fn store_created_dbcs(created_dbcs: Vec<Dbc>, wallet_dir: &Path) -> Result<()> {
    // The create dbcs dir within the wallet dir.
    let created_dbcs_path = wallet_dir.join(CREATED_DBCS_DIR_NAME);
    for dbc in created_dbcs.into_iter() {
        // One dir per recipient public address.
        // let public_address_name = public_address_name(dbc.public_address());
        // let public_address_dir = format!("public_address_{}", hex::encode(public_address_name));
        let dbc_id_name = *DbcAddress::from_dbc_id(&dbc.id()).xorname();
        let dbc_id_file_name = format!("{}.dbc", hex::encode(dbc_id_name));

        // let public_address_dir_path = created_dbcs_path.join(&public_address_dir);
        fs::create_dir_all(&created_dbcs_path).await?;

        let dbc_file_path = created_dbcs_path.join(dbc_id_file_name);

        let hex = dbc.to_hex().map_err(Error::Dbc)?;
        fs::write(dbc_file_path, &hex).await?;
    }
    Ok(())
}

/// Loads all the dbcs found in the received dbcs dir.
pub(super) async fn load_received_dbcs(wallet_dir: &Path) -> Result<Vec<Dbc>> {
    let received_dbcs_path = match std::env::var("RECEIVED_DBCS_PATH") {
        Ok(path) => PathBuf::from(path),
        Err(_) => wallet_dir.join(RECEIVED_DBCS_DIR_NAME),
    };

    let mut deposits = vec![];
    for entry in walkdir::WalkDir::new(&received_dbcs_path)
        .into_iter()
        .flatten()
    {
        if entry.file_type().is_file() {
            let file_name = entry.file_name();
            println!("Reading deposited tokens from {file_name:?}.");

            let dbc_data = fs::read_to_string(entry.path()).await?;
            let dbc = match Dbc::from_hex(dbc_data.trim()) {
                Ok(dbc) => dbc,
                Err(_) => {
                    println!(
                        "This file does not appear to have valid hex-encoded DBC data. \
                        Skipping it."
                    );
                    continue;
                }
            };

            deposits.push(dbc);
        }
    }

    if deposits.is_empty() {
        println!("No deposits found at {}.", received_dbcs_path.display());
    }

    Ok(deposits)
}

/// Loads a specific dbc from path
pub async fn load_dbc(dbc_id: &DbcId, wallet_dir: &Path) -> Option<Dbc> {
    let created_dbcs_path = wallet_dir.join(CREATED_DBCS_DIR_NAME);
    let dbc_id_name = *DbcAddress::from_dbc_id(dbc_id).xorname();
    let dbc_id_file_name = format!("{}.dbc", hex::encode(dbc_id_name));
    // Construct the path to the dbc file
    let dbc_file_path = created_dbcs_path.join(dbc_id_file_name);

    // Read the dbc data from the file
    match fs::read_to_string(dbc_file_path).await {
        Ok(dbc_data) => {
            // Convert the dbc data from hex to Dbc
            match Dbc::from_hex(dbc_data.trim()) {
                Ok(dbc) => Some(dbc),
                Err(error) => {
                    warn!("Failed to convert dbc data from hex: {}", error);
                    None
                }
            }
        }
        Err(error) => {
            warn!("Failed to read dbc file: {}", error);
            None
        }
    }
}
