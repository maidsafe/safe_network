// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    error::{Error, Result},
    public_address_name, KeyLessWallet,
};

use sn_dbc::Dbc;
use sn_protocol::storage::DbcAddress;
use std::path::Path;
use tokio::fs;

// Filename for storing a wallet.
const WALLET_FILE_NAME: &str = "wallet";
const CREATED_DBCS_DIR_NAME: &str = "created_dbcs";
const RECEIVED_DBCS_DIR_NAME: &str = "received_dbcs";

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

/// Hex encode and write each `Dbc` to a separate file in respective
/// recipient public address dir in the created dbcs dir. Each file is named after the dbc id.
pub(super) async fn store_created_dbcs(created_dbcs: Vec<Dbc>, wallet_dir: &Path) -> Result<()> {
    // The create dbcs dir within the wallet dir.
    let created_dbcs_path = wallet_dir.join(CREATED_DBCS_DIR_NAME);
    for dbc in created_dbcs.into_iter() {
        // One dir per recipient public address.
        let public_address_name = public_address_name(dbc.public_address());
        let public_address_dir = format!("public_address_{}", hex::encode(public_address_name));
        let dbc_id_name = *DbcAddress::from_dbc_id(&dbc.id()).name();
        let dbc_id_file_name = format!("{}.dbc", hex::encode(dbc_id_name));

        let public_address_dir_path = created_dbcs_path.join(&public_address_dir);
        fs::create_dir_all(&public_address_dir_path).await?;

        let dbc_file_path = public_address_dir_path.join(dbc_id_file_name);

        let hex = dbc.to_hex().map_err(|e| Error::Dbc(Box::new(e)))?;
        fs::write(dbc_file_path, &hex).await?;
    }
    Ok(())
}

/// Loads all the dbcs found in the received dbcs dir.
pub(super) async fn load_received_dbcs(wallet_dir: &Path) -> Result<Vec<Dbc>> {
    // The new dbcs dir within the wallet dir.
    let received_dbcs_path = wallet_dir.join(RECEIVED_DBCS_DIR_NAME);
    let mut deposits = vec![];

    for entry in walkdir::WalkDir::new(received_dbcs_path)
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
        println!("No deposits found.");
    }

    Ok(deposits)
}
