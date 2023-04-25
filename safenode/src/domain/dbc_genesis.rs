// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::wallet::LocalWallet;

use crate::client::{Client, WalletClient};

use sn_dbc::{
    rng, Dbc, DbcIdSource, DbcTransaction, Error as DbcError, Hash, InputHistory, MainKey,
    PublicAddress, RevealedAmount, RevealedInput, Token, TransactionBuilder,
};

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
const TOTAL_SUPPLY: u64 = u32::MAX as u64 * u64::pow(10, 9);

/// The secret key for the genesis DBC.
///
/// This key is public for auditing purposes. Hard coding its value means all nodes will be able to
/// validate it.
const GENESIS_DBC_SK: &str = "0c5152498fc5b2f9ed691ef875f2c16f1f950910391f7ba1df63e9f0ce4b2780";

/// Main error type for the crate.
#[derive(Error, Debug, Clone)]
pub enum Error {
    /// Error occurred when creating the Genesis DBC.
    #[error("Genesis DBC error:: {0}")]
    GenesisDbcError(String),
    /// The dbc error reason that parsing failed.
    #[error("Failed to parse reason: {0}")]
    FailedToParseReason(#[from] DbcError),
}

/// Create the genesis DBC.
/// The genesis DBC is the first DBC in the network. It is created without
/// a source transaction, as there was nothing before it.
pub(crate) fn create_genesis() -> GenesisResult<Dbc> {
    create_first_dbc_from_key(&genesis_key())
}

/// Returns a dbc with the requested number of tokens, for use by E2E test instances.
pub async fn get_tokens_from_faucet(amount: Token, to: PublicAddress, client: &Client) -> Dbc {
    send(load_faucet_wallet(client).await, amount, to, client).await
}

pub(crate) async fn send(
    from: LocalWallet,
    amount: Token,
    to: PublicAddress,
    client: &Client,
) -> Dbc {
    if amount.as_nano() == 0 {
        panic!("Amount must be more than zero.");
    }

    let mut wallet_client = WalletClient::new(client.clone(), from);
    let new_dbc = wallet_client
        .send(amount, to)
        .await
        .expect("Tokens shall be successfully sent.");

    let mut wallet = wallet_client.into_wallet();
    wallet
        .store()
        .await
        .expect("Wallet shall be successfully stored.");
    wallet
        .store_created_dbc(new_dbc.clone())
        .await
        .expect("Created dbc shall be successfully stored.");

    new_dbc
}

/// Load or create faucet wallet.
pub async fn load_faucet_wallet(client: &Client) -> LocalWallet {
    let genesis_wallet = load_genesis_wallet().await;

    println!("Loading faucet...");
    let mut faucet_wallet = create_faucet_wallet().await;

    use super::wallet::{DepositWallet, Wallet};
    let faucet_balance = faucet_wallet.balance();
    if faucet_balance.as_nano() > 0 {
        println!("Faucet wallet balance: {faucet_balance}");
        return faucet_wallet;
    }

    // Transfer to faucet. We will transfer almost all of the genesis wallet's
    // balance to the faucet, only leaving enough to pay for transfer fees.

    let initial_fee_margin = 500_000;
    let faucet_balance = Token::from_nano(genesis_wallet.balance().as_nano() - initial_fee_margin);
    println!("Sending {faucet_balance} from genesis to faucet wallet..");
    let tokens = send(
        genesis_wallet,
        faucet_balance,
        faucet_wallet.address(),
        client,
    )
    .await;

    faucet_wallet.deposit(vec![tokens]);
    faucet_wallet
        .store()
        .await
        .expect("Faucet wallet shall be stored successfully.");
    println!("Faucet wallet balance: {}", faucet_wallet.balance());

    faucet_wallet
}

async fn load_genesis_wallet() -> LocalWallet {
    println!("Loading genesis...");
    let mut genesis_wallet = create_genesis_wallet().await;
    let genesis_balance = genesis_wallet.balance();
    if genesis_balance.as_nano() > 0 {
        println!("Genesis wallet balance: {genesis_balance}");
        return genesis_wallet;
    }

    let genesis = create_genesis().expect("Genesis shall be created successfully.");

    use super::wallet::{DepositWallet, Wallet};
    genesis_wallet.deposit(vec![genesis]);
    genesis_wallet
        .store()
        .await
        .expect("Genesis wallet shall be stored successfully.");

    let genesis_balance = genesis_wallet.balance();
    println!("Genesis wallet balance: {genesis_balance}");

    genesis_wallet
}

async fn create_genesis_wallet() -> LocalWallet {
    let root_dir = get_genesis_dir().await;
    let wallet_dir = root_dir.join("wallet");
    tokio::fs::create_dir_all(&wallet_dir)
        .await
        .expect("Genesis wallet path to be successfully created.");

    let secret_key = bls::SecretKey::from_hex(GENESIS_DBC_SK)
        .expect("Genesis key hex shall be successfully parsed.");
    let main_key = MainKey::new(secret_key);
    let main_key_path = wallet_dir.join("main_key");
    tokio::fs::write(main_key_path, hex::encode(main_key.to_bytes()))
        .await
        .expect("Genesis key hex shall be successfully stored.");

    LocalWallet::load_from(&root_dir)
        .await
        .expect("Faucet wallet shall be created successfully.")
}

/// Create a first DBC given any key (i.e. not specifically the hard coded genesis key).
/// The derivation index and blinding factor are hard coded to ensure deterministic creation.
/// This is useful in tests.
pub(crate) fn create_first_dbc_from_key(first_dbc_key: &MainKey) -> GenesisResult<Dbc> {
    let dbc_id_src = DbcIdSource {
        public_address: first_dbc_key.public_address(),
        derivation_index: [0u8; 32],
    };
    let derived_key = first_dbc_key.derive_key(&dbc_id_src.derivation_index);
    let revealed_amount = RevealedAmount {
        value: GENESIS_DBC_AMOUNT,
        blinding_factor: sn_dbc::BlindingFactor::from_bits([0u8; 32]),
    };

    // Use the same key as the input and output of Genesis Tx.
    // The src tx is empty as this is the first DBC.
    let genesis_input = InputHistory {
        input: RevealedInput::new(derived_key, revealed_amount),
        input_src_tx: DbcTransaction {
            inputs: vec![],
            outputs: vec![],
        },
    };

    let reason = Hash::hash(b"GENESIS");

    let dbc_builder = TransactionBuilder::default()
        .add_input(genesis_input)
        .add_output(Token::from_nano(GENESIS_DBC_AMOUNT), dbc_id_src)
        .build(reason, rng::thread_rng())
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
) -> GenesisResult<Vec<(Dbc, RevealedAmount)>> {
    let rng = &mut rng::thread_rng();

    let derived_key = dbc.derived_key(main_key)?;
    let revealed_amount = dbc.revealed_amount(&derived_key)?;
    let input = InputHistory {
        input: RevealedInput::new(derived_key, revealed_amount),
        input_src_tx: dbc.src_tx.clone(),
    };

    let recipients: Vec<_> = (0..number)
        .map(|_| {
            let dbc_id_src = main_key.random_dbc_id_src(rng);
            let amount = revealed_amount.value() / number as u64;
            (Token::from_nano(amount), dbc_id_src)
        })
        .collect();

    let dbc_builder = TransactionBuilder::default()
        .add_input(input)
        .add_outputs(recipients)
        .build(Hash::default(), rng::thread_rng())
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

fn genesis_key() -> MainKey {
    let secret_key = bls::SecretKey::from_hex(GENESIS_DBC_SK)
        .expect("Genesis key hex shall be successfully parsed.");
    MainKey::new(secret_key)
}

async fn create_faucet_wallet() -> LocalWallet {
    let root_dir = get_faucet_dir().await;
    LocalWallet::load_from(&root_dir)
        .await
        .expect("Faucet wallet shall be created successfully.")
}

// We need deterministic and fix path for the faucet wallet.
// Otherwise the test instances will not be able to find the same faucet instance.
async fn get_genesis_dir() -> PathBuf {
    let mut home_dirs = dirs_next::home_dir().expect("A homedir to exist.");
    home_dirs.push(".safe");
    home_dirs.push("test_genesis");
    tokio::fs::create_dir_all(home_dirs.as_path())
        .await
        .expect("Genesis test path to be successfully created.");
    home_dirs
}

// We need deterministic and fix path for the faucet wallet.
// Otherwise the test instances will not be able to find the same faucet instance.
async fn get_faucet_dir() -> PathBuf {
    let mut home_dirs = dirs_next::home_dir().expect("A homedir to exist.");
    home_dirs.push(".safe");
    home_dirs.push("test_faucet");
    tokio::fs::create_dir_all(home_dirs.as_path())
        .await
        .expect("Faucet test path to be successfully created.");
    home_dirs
}
