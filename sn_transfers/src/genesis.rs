// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::wallet::HotWallet;

use crate::{
    wallet::Result as WalletResult, CashNote, DerivationIndex, Input, MainPubkey, MainSecretKey,
    NanoTokens, SignedSpend, SpendReason, Transaction, TransactionBuilder,
    TransferError as CashNoteError,
};

use bls::SecretKey;
use lazy_static::lazy_static;
use std::{fmt::Debug, path::PathBuf};
use thiserror::Error;

/// Number of tokens in the Genesis CashNote.
/// At the inception of the Network 30 % of total supply - i.e. 1,288,490,189 - whole tokens will be created.
/// Each whole token can be subdivided 10^9 times,
/// thus creating a total of 1,288,490,189,000,000,000 available units.
pub(super) const GENESIS_CASHNOTE_AMOUNT: u64 = (0.3 * TOTAL_SUPPLY as f64) as u64;

/// Based on the given store cost, it calculates what's the expected amount to be paid as network royalties.
/// Network royalties fee is expected to be 15% of the payment amount, i.e. 85% of store cost + 15% royalties fees.
pub fn calculate_royalties_fee(store_cost: NanoTokens) -> NanoTokens {
    let fees_amount = (store_cost.as_nano() as f64 * 0.15) / 0.85;
    // we round down the calculated amount
    NanoTokens::from(fees_amount as u64)
}

/// A specialised `Result` type for genesis crate.
pub(super) type GenesisResult<T> = Result<T, Error>;

/// Total supply of tokens that will eventually exist in the network: 4,294,967,295 * 10^9 = 4,294,967,295,000,000,000.
pub const TOTAL_SUPPLY: u64 = u32::MAX as u64 * u64::pow(10, 9);

/// Main error type for the crate.
#[derive(Error, Debug, Clone)]
pub enum Error {
    /// Error occurred when creating the Genesis CashNote.
    #[error("Genesis CashNote error:: {0}")]
    GenesisCashNoteError(String),
    /// The cash_note error reason that parsing failed.
    #[error("Failed to parse reason: {0}")]
    FailedToParseReason(#[from] Box<CashNoteError>),

    #[error("Failed to perform wallet action: {0}")]
    WalletError(String),
}

lazy_static! {
    /// This key is public for auditing purposes.
    /// The hard coded value is for production release, allows all nodes to validate it.
    /// The env set value is only used for testing purpose.
    pub static ref GENESIS_PK: MainPubkey = {
        let pk_str = std::env::var("GENESIS_PK").unwrap_or("96d3f6fb55ab504307d56f4085856dc61806ca5285eba1d8b9d1ce83db2604b41de9f2f50a0ea3dd160b65c1e8798b43".to_string());  // DevSkim: ignore DS173237

        match MainPubkey::from_hex(pk_str) {
            Ok(pk) => pk,
            Err(err) => panic!("Failed to parse genesis PK: {err:?}"),
        }
    };
}

lazy_static! {
    /// Unlike the `GENESIS_PK`, the hard coded secret_key is for testing purpose.
    /// The one for live network shall be passed in via env set.
    static ref GENESIS_SK_STR: String = {
        std::env::var("GENESIS_SK").unwrap_or("141a4ccbce0ef0992c3db01ad2215f89ff5249c0d6749d979f37745c3c0170c9".to_string())  // DevSkim: ignore DS173237
    };
}

lazy_static! {
    /// Load the genesis CashNote.
    /// The genesis CashNote is the first CashNote in the network. It is created without
    /// a source transaction, as there was nothing before it.
    pub static ref GENESIS_CASHNOTE: CashNote = {
        match create_first_cash_note_from_key(&get_genesis_sk()) {
            Ok(cash_note) => cash_note,
            Err(err) => panic!("Failed to create genesis CashNote: {err:?}"),
        }
    };
}

/// Returns genesis SK (normally for testing purpose).
pub fn get_genesis_sk() -> MainSecretKey {
    match SecretKey::from_hex(&GENESIS_SK_STR) {
        Ok(sk) => MainSecretKey::new(sk),
        Err(err) => panic!("Failed to parse genesis SK: {err:?}"),
    }
}

/// Return if provided Transaction is genesis parent tx.
pub fn is_genesis_parent_tx(parent_tx: &Transaction) -> bool {
    parent_tx == &GENESIS_CASHNOTE.parent_tx
}

/// Return if provided Spend is genesis spend.
pub fn is_genesis_spend(spend: &SignedSpend) -> bool {
    let bytes = spend.spend.to_bytes_for_signing();
    spend.spend.unique_pubkey == GENESIS_CASHNOTE.unique_pubkey()
        && GENESIS_CASHNOTE
            .unique_pubkey()
            .verify(&spend.derived_key_sig, bytes)
        && is_genesis_parent_tx(&spend.spend.parent_tx)
        && spend.spend.amount == NanoTokens::from(GENESIS_CASHNOTE_AMOUNT)
}

pub fn load_genesis_wallet() -> Result<HotWallet, Error> {
    info!("Loading genesis...");
    if let Ok(wallet) = get_existing_genesis_wallet() {
        return Ok(wallet);
    }

    let mut genesis_wallet = create_genesis_wallet();

    info!(
        "Depositing genesis CashNote: {:?}",
        GENESIS_CASHNOTE.unique_pubkey()
    );
    genesis_wallet
        .deposit_and_store_to_disk(&vec![GENESIS_CASHNOTE.clone()])
        .map_err(|err| Error::WalletError(err.to_string()))
        .expect("Genesis wallet shall be stored successfully.");

    let genesis_balance = genesis_wallet.balance();
    info!("Genesis wallet balance: {genesis_balance}");

    Ok(genesis_wallet)
}

fn create_genesis_wallet() -> HotWallet {
    let root_dir = get_genesis_dir();
    let wallet_dir = root_dir.join("wallet");
    std::fs::create_dir_all(&wallet_dir).expect("Genesis wallet path to be successfully created.");

    crate::wallet::store_new_keypair(&wallet_dir, &get_genesis_sk())
        .expect("Genesis key shall be successfully stored.");

    HotWallet::load_from(&root_dir)
        .expect("Faucet wallet (after genesis) shall be created successfully.")
}

fn get_existing_genesis_wallet() -> WalletResult<HotWallet> {
    let root_dir = get_genesis_dir();

    let mut wallet = HotWallet::load_from(&root_dir)?;
    wallet.try_load_cash_notes()?;

    Ok(wallet)
}

/// Create a first CashNote given any key (i.e. not specifically the hard coded genesis key).
/// The derivation index is hard coded to ensure deterministic creation.
/// This is useful in tests.
pub fn create_first_cash_note_from_key(
    first_cash_note_key: &MainSecretKey,
) -> GenesisResult<CashNote> {
    let main_pubkey = first_cash_note_key.main_pubkey();
    debug!("genesis cashnote main_pubkey: {:?}", main_pubkey);
    let derivation_index = DerivationIndex([0u8; 32]);
    let derived_key = first_cash_note_key.derive_key(&derivation_index);

    // Use the same key as the input and output of Genesis Tx.
    // The src tx is empty as this is the first CashNote.
    let genesis_input = Input {
        unique_pubkey: derived_key.unique_pubkey(),
        amount: NanoTokens::from(GENESIS_CASHNOTE_AMOUNT),
    };

    let reason = SpendReason::default();

    let cash_note_builder = TransactionBuilder::default()
        .add_input(
            genesis_input,
            Some(derived_key),
            Transaction::empty(),
            derivation_index,
        )
        .add_output(
            NanoTokens::from(GENESIS_CASHNOTE_AMOUNT),
            main_pubkey,
            derivation_index,
        )
        .build(reason, vec![])
        .map_err(|err| {
            Error::GenesisCashNoteError(format!(
                "Failed to build the CashNote transaction for genesis CashNote: {err}",
            ))
        })?;

    // build the output CashNotes
    let output_cash_notes = cash_note_builder.build_without_verifying().map_err(|err| {
        Error::GenesisCashNoteError(format!(
            "CashNote builder failed to create output genesis CashNote: {err}",
        ))
    })?;

    // just one output CashNote is expected which is the genesis CashNote
    let (genesis_cash_note, _) = output_cash_notes.into_iter().next().ok_or_else(|| {
        Error::GenesisCashNoteError(
            "CashNote builder (unexpectedly) contains an empty set of outputs.".to_string(),
        )
    })?;

    Ok(genesis_cash_note)
}

// We need deterministic and fix path for the faucet wallet.
// Otherwise the test instances will not be able to find the same faucet instance.
pub fn get_faucet_data_dir() -> PathBuf {
    let mut data_dirs = dirs_next::data_dir().expect("A homedir to exist.");
    data_dirs.push("safe");
    data_dirs.push("test_faucet");
    std::fs::create_dir_all(data_dirs.as_path())
        .expect("Faucet test path to be successfully created.");
    data_dirs
}

// We need deterministic and fix path for the genesis wallet.
// Otherwise the test instances will not be able to find the same genesis instance.
fn get_genesis_dir() -> PathBuf {
    let mut data_dirs = dirs_next::data_dir().expect("A homedir to exist.");
    data_dirs.push("safe");
    data_dirs.push("test_genesis");
    std::fs::create_dir_all(data_dirs.as_path())
        .expect("Genesis test path to be successfully created.");
    data_dirs
}
