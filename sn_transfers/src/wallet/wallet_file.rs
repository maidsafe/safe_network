// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    error::{Error, Result},
    KeyLessWallet,
};
use crate::{CashNote, SignedSpend, SpendAddress, UniquePubkey};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

// Filename for storing a wallet.
const WALLET_FILE_NAME: &str = "wallet";
const WALLET_LOCK_FILE_NAME: &str = "wallet.lock";
const CASHNOTES_DIR_NAME: &str = "cash_notes";
const UNCONFIRMED_TX_NAME: &str = "unconfirmed_spend_requests";

/// Writes the `KeyLessWallet` to the specified path.
pub(super) fn store_wallet(wallet_dir: &Path, wallet: &KeyLessWallet) -> Result<()> {
    let wallet_path = wallet_dir.join(WALLET_FILE_NAME);
    let mut file = fs::File::create(wallet_path)?;
    let mut serialiser = rmp_serde::encode::Serializer::new(&mut file);
    wallet.serialize(&mut serialiser)?;
    Ok(())
}

/// Returns the wallet filename
pub(super) fn wallet_file_name(wallet_dir: &Path) -> PathBuf {
    wallet_dir.join(WALLET_FILE_NAME)
}

/// Returns the wallet lockfile filename
pub fn wallet_lockfile_name(wallet_dir: &Path) -> PathBuf {
    wallet_dir.join(WALLET_LOCK_FILE_NAME)
}

/// Writes the `unconfirmed_spend_requests` to the specified path.
pub(super) fn store_unconfirmed_spend_requests(
    wallet_dir: &Path,
    unconfirmed_spend_requests: &BTreeSet<SignedSpend>,
) -> Result<()> {
    let unconfirmed_spend_requests_path = wallet_dir.join(UNCONFIRMED_TX_NAME);

    let mut file = fs::File::create(unconfirmed_spend_requests_path)?;
    let mut serialiser = rmp_serde::encode::Serializer::new(&mut file);
    unconfirmed_spend_requests.serialize(&mut serialiser)?;
    Ok(())
}

/// Remove the `unconfirmed_spend_requests` from the specified path.
pub(super) fn remove_unconfirmed_spend_requests(
    wallet_dir: &Path,
    _unconfirmed_spend_requests: &BTreeSet<SignedSpend>,
) -> Result<()> {
    let unconfirmed_spend_requests_path = wallet_dir.join(UNCONFIRMED_TX_NAME);

    debug!("Removing unconfirmed_spend_requests from {unconfirmed_spend_requests_path:?}");
    fs::remove_file(unconfirmed_spend_requests_path)?;
    Ok(())
}

/// Returns `Some(Vec<SpendRequest>)` or None if file doesn't exist.
pub(super) fn get_unconfirmed_spend_requests(
    wallet_dir: &Path,
) -> Result<Option<BTreeSet<SignedSpend>>> {
    let path = wallet_dir.join(UNCONFIRMED_TX_NAME);
    if !path.is_file() {
        return Ok(None);
    }

    let file = fs::File::open(&path)?;
    let unconfirmed_spend_requests = rmp_serde::from_read(&file)?;

    Ok(Some(unconfirmed_spend_requests))
}

/// Hex encode and write each `CashNote` to a separate file in respective
/// recipient public address dir in the created cash_notes dir. Each file is named after the cash_note id.
pub(super) fn store_created_cash_notes<'a, T>(
    created_cash_notes: T,
    wallet_dir: &Path,
) -> Result<()>
where
    T: IntoIterator<Item = &'a CashNote>,
{
    // The create cash_notes dir within the wallet dir.
    let created_cash_notes_path = wallet_dir.join(CASHNOTES_DIR_NAME);
    for cash_note in created_cash_notes {
        let unique_pubkey_name =
            *SpendAddress::from_unique_pubkey(&cash_note.unique_pubkey()).xorname();
        let unique_pubkey_file_name = format!("{}.cash_note", hex::encode(unique_pubkey_name));

        fs::create_dir_all(&created_cash_notes_path)?;

        let cash_note_file_path = created_cash_notes_path.join(unique_pubkey_file_name);
        debug!("Writing cash note to: {cash_note_file_path:?}");

        let hex = cash_note
            .to_hex()
            .map_err(|_| Error::FailedToHexEncodeCashNote)?;
        fs::write(cash_note_file_path, &hex)?;
    }
    Ok(())
}

/// Hex encode and remove each `CashNote` from a separate file in respective
pub(super) fn remove_cash_notes<'a, T>(cash_notes: T, wallet_dir: &Path) -> Result<()>
where
    T: IntoIterator<Item = &'a UniquePubkey>,
{
    // The create cash_notes dir within the wallet dir.
    let created_cash_notes_path = wallet_dir.join(CASHNOTES_DIR_NAME);
    for cash_note_key in cash_notes {
        let unique_pubkey_name = *SpendAddress::from_unique_pubkey(cash_note_key).xorname();
        let unique_pubkey_file_name = format!("{}.cash_note", hex::encode(unique_pubkey_name));

        debug!("Removing cash note from: {:?}", created_cash_notes_path);

        let cash_note_file_path = created_cash_notes_path.join(unique_pubkey_file_name);

        fs::remove_file(cash_note_file_path)?;
    }
    Ok(())
}

/// Loads all the cash_notes found in the cash_notes dir.
pub(super) fn load_cash_notes_from_disk(wallet_dir: &Path) -> Result<Vec<CashNote>> {
    let cash_notes_path = match std::env::var("CASHNOTES_PATH") {
        Ok(path) => PathBuf::from(path),
        Err(_) => wallet_dir.join(CASHNOTES_DIR_NAME),
    };

    let mut deposits = vec![];
    for entry in walkdir::WalkDir::new(&cash_notes_path)
        .into_iter()
        .flatten()
    {
        if entry.file_type().is_file() {
            let file_name = entry.file_name();
            println!("Reading deposited tokens from {file_name:?}.");

            let cash_note_data = fs::read_to_string(entry.path())?;
            let cash_note = match CashNote::from_hex(cash_note_data.trim()) {
                Ok(cash_note) => cash_note,
                Err(_) => {
                    println!(
                        "This file does not appear to have valid hex-encoded CashNote data. \
                        Skipping it."
                    );
                    continue;
                }
            };

            deposits.push(cash_note);
        }
    }

    if deposits.is_empty() {
        println!("No deposits found at {}.", cash_notes_path.display());
    }

    Ok(deposits)
}

/// Loads a specific cash_note from path
pub fn load_created_cash_note(unique_pubkey: &UniquePubkey, wallet_dir: &Path) -> Option<CashNote> {
    trace!("Loading cash_note from file with pubkey: {unique_pubkey:?}");
    let created_cash_notes_path = wallet_dir.join(CASHNOTES_DIR_NAME);
    let unique_pubkey_name = *SpendAddress::from_unique_pubkey(unique_pubkey).xorname();
    let unique_pubkey_file_name = format!("{}.cash_note", hex::encode(unique_pubkey_name));
    // Construct the path to the cash_note file
    let cash_note_file_path = created_cash_notes_path.join(unique_pubkey_file_name);

    // Read the cash_note data from the file
    match fs::read_to_string(cash_note_file_path.clone()) {
        Ok(cash_note_data) => {
            // Convert the cash_note data from hex to CashNote
            match CashNote::from_hex(cash_note_data.trim()) {
                Ok(cash_note) => Some(cash_note),
                Err(error) => {
                    warn!("Failed to convert cash_note data from hex: {}", error);
                    None
                }
            }
        }
        Err(error) => {
            warn!(
                "Failed to read cash_note file {:?}: {}",
                cash_note_file_path, error
            );
            None
        }
    }
}
