// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::wallet::LocalWallet;

#[cfg(test)]
use sn_dbc::{random_derivation_index, rng};

use sn_dbc::{
    Dbc, DbcTransaction, Error as DbcError, Hash, Input, MainKey, Token, TransactionBuilder,
};

use bls::SecretKey;
use lazy_static::lazy_static;
use std::{fmt::Debug, path::PathBuf};
use thiserror::Error;

/// Number of tokens in the Genesis DBC.
/// At the inception of the Network 30 % of total supply - i.e. 1,288,490,189 - whole tokens will be created.
/// Each whole token can be subdivided 10^9 times,
/// thus creating a total of 1,288,490,189,000,000,000 available units.
pub(super) const GENESIS_DBC_AMOUNT: u64 = (0.3 * TOTAL_SUPPLY as f64) as u64;

/// A specialised `Result` type for dbc_genesis crate.
pub(super) type GenesisResult<T> = Result<T, Error>;

/// Total supply of tokens that will eventually exist in the network: 4,294,967,295 * 10^9 = 4,294,967,295,000,000,000.
pub const TOTAL_SUPPLY: u64 = u32::MAX as u64 * u64::pow(10, 9);

/// The secret key for the genesis DBC.
///
/// This key is public for auditing purposes. Hard coding its value means all nodes will be able to
/// validate it.
const GENESIS_DBC_SK: &str = "5f15ae2ea589007e1474e049bbc32904d583265f12ce1f8153f955076a9af49b";

/// Main error type for the crate.
#[derive(Error, Debug, Clone)]
pub enum Error {
    /// Error occurred when creating the Genesis DBC.
    #[error("Genesis DBC error:: {0}")]
    GenesisDbcError(String),
    /// The dbc error reason that parsing failed.
    #[error("Failed to parse reason: {0}")]
    FailedToParseReason(#[from] Box<DbcError>),

    #[error("Failed to perform wallet action: {0}")]
    WalletError(String),
}

lazy_static! {
    /// Load the genesis DBC.
    /// The genesis DBC is the first DBC in the network. It is created without
    /// a source transaction, as there was nothing before it.
    pub static ref GENESIS_DBC: Dbc = {
        let main_key = match SecretKey::from_hex(GENESIS_DBC_SK) {
            Ok(sk) => MainKey::new(sk),
            Err(err) => panic!("Failed to parse hard-coded genesis DBC SK: {err:?}"),
        };

        match create_first_dbc_from_key(&main_key) {
            Ok(dbc) => dbc,
            Err(err) => panic!("Failed to create genesis DBC: {err:?}"),
        }
    };
}

/// Return if provided DbcTransaction is genesis parent tx.
pub fn is_genesis_parent_tx(parent_tx: &DbcTransaction) -> bool {
    parent_tx == &GENESIS_DBC.src_tx
}

pub fn load_genesis_wallet() -> Result<LocalWallet, Error> {
    info!("Loading genesis...");
    let mut genesis_wallet = create_genesis_wallet();

    info!("Depositing genesis DBC: {:#?}", GENESIS_DBC.id());
    genesis_wallet
        .deposit(&vec![GENESIS_DBC.clone()])
        .map_err(|err| Error::WalletError(err.to_string()))?;
    genesis_wallet
        .store()
        .expect("Genesis wallet shall be stored successfully.");

    let genesis_balance = genesis_wallet.balance();
    info!("Genesis wallet balance: {genesis_balance}");

    Ok(genesis_wallet)
}

pub fn create_genesis_wallet() -> LocalWallet {
    let root_dir = get_genesis_dir();
    let wallet_dir = root_dir.join("wallet");
    std::fs::create_dir_all(&wallet_dir).expect("Genesis wallet path to be successfully created.");

    let secret_key = bls::SecretKey::from_hex(GENESIS_DBC_SK)
        .expect("Genesis key hex shall be successfully parsed.");
    let main_key = MainKey::new(secret_key);
    let main_key_path = wallet_dir.join("main_key");
    std::fs::write(main_key_path, hex::encode(main_key.to_bytes()))
        .expect("Genesis key hex shall be successfully stored.");

    LocalWallet::load_from(&root_dir)
        .expect("Faucet wallet (after genesis) shall be created successfully.")
}

/// Create a first DBC given any key (i.e. not specifically the hard coded genesis key).
/// The derivation index and blinding factor are hard coded to ensure deterministic creation.
/// This is useful in tests.
pub(crate) fn create_first_dbc_from_key(first_dbc_key: &MainKey) -> GenesisResult<Dbc> {
    let public_address = first_dbc_key.public_address();
    let derivation_index = [0u8; 32];
    let derived_key = first_dbc_key.derive_key(&derivation_index);

    // Use the same key as the input and output of Genesis Tx.
    // The src tx is empty as this is the first DBC.
    let genesis_input = Input {
        dbc_id: derived_key.dbc_id(),
        token: Token::from_nano(GENESIS_DBC_AMOUNT),
    };

    let reason = Hash::hash(b"GENESIS");

    let dbc_builder = TransactionBuilder::default()
        .add_input(genesis_input, derived_key, DbcTransaction::empty())
        .add_output(
            Token::from_nano(GENESIS_DBC_AMOUNT),
            public_address,
            derivation_index,
        )
        .build(reason)
        .map_err(|err| {
            Error::GenesisDbcError(format!(
                "Failed to build the DBC transaction for genesis DBC: {err}",
            ))
        })?;

    // build the output DBCs
    let output_dbcs = dbc_builder.build_without_verifying().map_err(|err| {
        Error::GenesisDbcError(format!(
            "DBC builder failed to create output genesis DBC: {err}",
        ))
    })?;

    // just one output DBC is expected which is the genesis DBC
    let (genesis_dbc, _) = output_dbcs.into_iter().next().ok_or_else(|| {
        Error::GenesisDbcError(
            "DBC builder (unexpectedly) contains an empty set of outputs.".to_string(),
        )
    })?;

    Ok(genesis_dbc)
}

/// Split a dbc into multiple. ONLY FOR TEST.
#[cfg(test)]
#[allow(clippy::result_large_err, unused)]
pub(super) fn split(
    dbc: &Dbc,
    main_key: &MainKey,
    number: usize,
) -> GenesisResult<Vec<(Dbc, Token)>> {
    let rng = &mut rng::thread_rng();

    let derived_key = dbc
        .derived_key(main_key)
        .map_err(|e| Error::FailedToParseReason(Box::new(e)))?;
    let token = dbc
        .token()
        .map_err(|e| Error::FailedToParseReason(Box::new(e)))?;
    let input = Input {
        dbc_id: dbc.id(),
        token,
    };

    let recipients: Vec<_> = (0..number)
        .map(|_| {
            let amount = token.as_nano() / number as u64;
            (
                sn_dbc::Token::from_nano(amount),
                main_key.public_address(),
                random_derivation_index(rng),
            )
        })
        .collect();

    let dbc_builder = TransactionBuilder::default()
        .add_input(input, derived_key, dbc.src_tx.clone())
        .add_outputs(recipients)
        .build(Hash::default())
        .map_err(|err| {
            Error::GenesisDbcError(format!(
                "Failed to build the DBC transaction for genesis DBC: {err}",
            ))
        })?;

    // build the output DBCs
    let output_dbcs = dbc_builder.build_without_verifying().map_err(|err| {
        Error::GenesisDbcError(format!(
            "DBC builder failed to create output genesis DBC: {err}",
        ))
    })?;

    Ok(output_dbcs)
}

pub fn create_faucet_wallet() -> LocalWallet {
    let root_dir = get_faucet_dir();

    println!("Loading faucet wallet... {:#?}", root_dir);
    LocalWallet::load_from(&root_dir).expect("Faucet wallet shall be created successfully.")
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

// We need deterministic and fix path for the faucet wallet.
// Otherwise the test instances will not be able to find the same faucet instance.
fn get_faucet_dir() -> PathBuf {
    let mut data_dirs = dirs_next::data_dir().expect("A homedir to exist.");
    data_dirs.push("safe");
    data_dirs.push("test_faucet");
    std::fs::create_dir_all(data_dirs.as_path())
        .expect("Faucet test path to be successfully created.");
    data_dirs
}
