// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::wallet::HotWallet;

use crate::{
    wallet::Result as WalletResult, CashNote, DerivationIndex, MainPubkey, MainSecretKey,
    NanoTokens, SignedSpend, Spend, SpendReason, TransferError, UniquePubkey,
};

use bls::SecretKey;
use lazy_static::lazy_static;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
    path::PathBuf,
};
use thiserror::Error;

/// Number of tokens in the Genesis CashNote.
/// At the inception of the Network 30 % of total supply - i.e. 1,288,490,189 - whole tokens will be created.
/// Each whole token can be subdivided 10^9 times,
/// thus creating a total of 1,288,490,189,000,000,000 available units.
pub(super) const GENESIS_CASHNOTE_AMOUNT: u64 = (0.3 * TOTAL_SUPPLY as f64) as u64;

/// The input derivation index for the genesis Spend.
pub const GENESIS_INPUT_DERIVATION_INDEX: DerivationIndex = DerivationIndex([0u8; 32]);
/// The output derivation index for the genesis Spend.
pub const GENESIS_OUTPUT_DERIVATION_INDEX: DerivationIndex = DerivationIndex([1u8; 32]);

/// Default genesis SK for testing purpose. Be sure to pass the correct `GENESIS_SK` value via env for release.
const DEFAULT_LIVE_GENESIS_SK: &str =
    "23746be7fa5df26c3065eb7aa26860981e435c1853cafafe472417bc94f340e9"; // DevSkim: ignore DS173237

/// Default genesis PK for testing purposes. Be sure to pass the correct `GENESIS_PK` value via env for release.
const DEFAULT_LIVE_GENESIS_PK: &str = "9934c21469a68415e6b06a435709e16bff6e92bf302aeb0ea9199d2d06a55f1b1a21e155853d3f94ae31f8f313f886ee"; // DevSkim: ignore DS173237

/// MIN_STORE_COST is 1, hence to have a MIN_ROYALTY_FEE to avoid zero royalty_fee.
const MIN_ROYALTY_FEE: u64 = 1;

/// Based on the given store cost, it calculates what's the expected amount to be paid as network royalties.
/// Network royalties fee is expected to be 15% of the payment amount, i.e. 85% of store cost + 15% royalties fees.
pub fn calculate_royalties_fee(store_cost: NanoTokens) -> NanoTokens {
    let fees_amount = std::cmp::max(
        MIN_ROYALTY_FEE,
        ((store_cost.as_nano() as f64 * 0.15) / 0.85) as u64,
    );
    // we round down the calculated amount
    NanoTokens::from(fees_amount)
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
    FailedToParseReason(#[from] Box<TransferError>),

    #[error("Failed to perform wallet action: {0}")]
    WalletError(String),
}

lazy_static! {
    pub static ref GENESIS_PK: MainPubkey = {
        let compile_time_key = option_env!("GENESIS_PK").unwrap_or(DEFAULT_LIVE_GENESIS_PK);
        let runtime_key =
            std::env::var("GENESIS_PK").unwrap_or_else(|_| compile_time_key.to_string());

        if runtime_key == DEFAULT_LIVE_GENESIS_PK {
            warn!("USING DEFAULT GENESIS SK (9934c2) FOR TESTING PURPOSES! EXPECTING PAIRED SK (23746b) TO BE USED!");
        } else if runtime_key == compile_time_key {
            warn!("Using compile-time GENESIS_PK: {}", compile_time_key);
        } else {
            warn!("Overridden by runtime GENESIS_PK: {}", runtime_key);
        }

        match MainPubkey::from_hex(&runtime_key) {
            Ok(pk) => {
                info!("Genesis PK: {pk:?}");
                pk
            }
            Err(err) => panic!("Failed to parse genesis PK: {err:?}"),
        }
    };
}

lazy_static! {
    /// This is the unique key for the genesis Spend
    pub static ref GENESIS_SPEND_UNIQUE_KEY: UniquePubkey = GENESIS_PK.new_unique_pubkey(&GENESIS_OUTPUT_DERIVATION_INDEX);
}

lazy_static! {
    pub static ref GENESIS_SK_STR: String = {
        let compile_time_key = option_env!("GENESIS_SK").unwrap_or(DEFAULT_LIVE_GENESIS_SK);
        let runtime_key =
            std::env::var("GENESIS_SK").unwrap_or_else(|_| compile_time_key.to_string());

        if runtime_key == DEFAULT_LIVE_GENESIS_SK {
            warn!("USING DEFAULT GENESIS SK (23746b) FOR TESTING PURPOSES! EXPECTING PAIRED PK (9934c2) TO BE USED!");
        } else if runtime_key == compile_time_key {
            warn!("Using compile-time GENESIS_SK");
        } else {
            warn!("Overridden by runtime GENESIS_SK");
        }

        runtime_key
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

/// Return if provided Spend is genesis spend.
pub fn is_genesis_spend(spend: &SignedSpend) -> bool {
    let bytes = spend.spend.to_bytes_for_signing();
    spend.spend.unique_pubkey == *GENESIS_SPEND_UNIQUE_KEY
        && GENESIS_SPEND_UNIQUE_KEY.verify(&spend.derived_key_sig, bytes)
        && spend.spend.amount() == NanoTokens::from(GENESIS_CASHNOTE_AMOUNT)
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

    crate::wallet::store_new_keypair(&wallet_dir, &get_genesis_sk(), None)
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
    let input_sk = first_cash_note_key.derive_key(&GENESIS_INPUT_DERIVATION_INDEX);
    let input_pk = input_sk.unique_pubkey();
    let output_pk = main_pubkey.new_unique_pubkey(&GENESIS_OUTPUT_DERIVATION_INDEX);
    let amount = NanoTokens::from(GENESIS_CASHNOTE_AMOUNT);

    let pre_genesis_spend = Spend {
        unique_pubkey: input_pk,
        reason: SpendReason::default(),
        ancestors: BTreeSet::new(),
        descendants: BTreeMap::from_iter([(output_pk, amount)]),
        royalties: vec![],
    };
    let parent_spends = BTreeSet::from_iter([SignedSpend::sign(pre_genesis_spend, &input_sk)]);

    let genesis_cash_note = CashNote {
        parent_spends,
        main_pubkey,
        derivation_index: GENESIS_OUTPUT_DERIVATION_INDEX,
    };

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_genesis() {
        for _ in 0..10 {
            let sk = bls::SecretKey::random();
            let sk_str = sk.to_hex();
            let genesis_sk = MainSecretKey::new(sk);
            let main_pubkey = genesis_sk.main_pubkey();

            let genesis_cn = match create_first_cash_note_from_key(&genesis_sk) {
                Ok(cash_note) => cash_note,
                Err(err) => panic!("Failed to create genesis CashNote: {err:?}"),
            };

            println!("=============================");
            println!("secret_key: {sk_str:?}");
            println!("main_pub_key: {:?}", main_pubkey.to_hex());
            println!(
                "genesis_cn.unique_pubkey: {:?}",
                genesis_cn.unique_pubkey().to_hex()
            );
        }
    }
}
