// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{CashNote, SignedSpend, SpendAddress, UniquePubkey};

use super::{
    error::{Error, Result},
    KeyLessWallet,
};

use std::{collections::BTreeSet, fs, path::Path};

// Filename for storing a wallet.
const WALLET_FILE_NAME: &str = "wallet";
const CASHNOTES_DIR_NAME: &str = "cash_notes";
const UNCONFRIMED_TX_NAME: &str = "unconfirmed_spend_requests";

/// Writes the `KeyLessWallet` to the specified path.
pub(super) fn store_wallet(wallet_dir: &Path, wallet: &KeyLessWallet) -> Result<()> {
    let wallet_path = wallet_dir.join(WALLET_FILE_NAME);
    let bytes = bincode::serialize(&wallet)?;
    fs::write(wallet_path, bytes)?;
    Ok(())
}

/// Returns `Some(KeyLessWallet)` or None if file doesn't exist.
pub(super) fn get_wallet(wallet_dir: &Path) -> Result<Option<KeyLessWallet>> {
    let path = wallet_dir.join(WALLET_FILE_NAME);
    if !path.is_file() {
        return Ok(None);
    }

    let bytes = fs::read(&path)?;
    let wallet = bincode::deserialize(&bytes)?;

    Ok(Some(wallet))
}

/// Writes the `unconfirmed_spend_requests` to the specified path.
pub(super) fn store_unconfirmed_spend_requests(
    wallet_dir: &Path,
    unconfirmed_spend_requests: &BTreeSet<SignedSpend>,
) -> Result<()> {
    let unconfirmed_spend_requests_path = wallet_dir.join(UNCONFRIMED_TX_NAME);
    let bytes = bincode::serialize(&unconfirmed_spend_requests)?;
    fs::write(unconfirmed_spend_requests_path, bytes)?;
    Ok(())
}

/// Returns `Some(Vec<SpendRequest>)` or None if file doesn't exist.
pub(super) fn get_unconfirmed_spend_requests(
    wallet_dir: &Path,
) -> Result<Option<BTreeSet<SignedSpend>>> {
    let path = wallet_dir.join(UNCONFRIMED_TX_NAME);
    if !path.is_file() {
        return Ok(None);
    }

    let bytes = fs::read(&path)?;
    let unconfirmed_spend_requests = bincode::deserialize(&bytes)?;

    Ok(Some(unconfirmed_spend_requests))
}

/// Hex encode and write each `CashNote` to a separate file in respective
/// recipient public address dir in the created cash_notes dir. Each file is named after the cash_note id.
pub(super) fn store_created_cash_notes(
    created_cash_notes: Vec<&CashNote>,
    wallet_dir: &Path,
) -> Result<()> {
    // The create cash_notes dir within the wallet dir.
    let created_cash_notes_path = wallet_dir.join(CASHNOTES_DIR_NAME);
    for cash_note in created_cash_notes.iter() {
        let unique_pubkey_name =
            *SpendAddress::from_unique_pubkey(&cash_note.unique_pubkey()).xorname();
        let unique_pubkey_file_name = format!("{}.cash_note", hex::encode(unique_pubkey_name));

        debug!("Writing cash note to: {:?}", created_cash_notes_path);
        fs::create_dir_all(&created_cash_notes_path)?;

        let cash_note_file_path = created_cash_notes_path.join(unique_pubkey_file_name);

        let hex = cash_note
            .to_hex()
            .map_err(|_| Error::FailedToHexEncodeCashNote)?;
        fs::write(cash_note_file_path, &hex)?;
    }
    Ok(())
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
