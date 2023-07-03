// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::wallet::LocalWallet;

#[cfg(test)]
use sn_dbc::{random_derivation_index, rng, Hash, Input, Token, TransactionBuilder};

use sn_dbc::{Dbc, DbcTransaction, Error as DbcError, MainKey};

use lazy_static::lazy_static;
use std::{fmt::Debug, path::PathBuf};
use thiserror::Error;

/// Number of tokens in the Genesis DBC.
/// At the inception of the Network 30 % of total supply - i.e. 1,288,490,189 - whole tokens will be created.
/// Each whole token can be subdivided 10^9 times,
/// thus creating a total of 1,288,490,189,000,000,000 available units.
#[cfg(test)]
pub(super) const GENESIS_DBC_AMOUNT: u64 = (0.3 * TOTAL_SUPPLY as f64) as u64;

/// A specialised `Result` type for dbc_genesis crate.
#[cfg(test)]
pub(super) type GenesisResult<T> = Result<T, Error>;

/// Total supply of tokens that will eventually exist in the network: 4,294,967,295 * 10^9 = 4,294,967,295,000,000,000.
#[cfg(test)]
const TOTAL_SUPPLY: u64 = u32::MAX as u64 * u64::pow(10, 9);

/// The secret key for the genesis DBC.
///
/// This key is public for auditing purposes. Hard coding its value means all nodes will be able to
/// validate it.
const GENESIS_DBC_SK: &str = "5f15ae2ea589007e1474e049bbc32904d583265f12ce1f8153f955076a9af49b";
const GENESIS_DBC_HEX: &str = "000000000000000049dff9aaca1dc4720c0b0d3311e95ca7cabf564d494c160573b2f3142d46fa25e9e34ca90982e4f35ad18fa8fc9126044deb21fb1ad88b77541e9c63b83fa932369f1a93f8139482b52f65dbb80c5c1c696cf30875bf3894022dd61ccdc1d88da83a50b3ce1f8fa29ba65c72b767f7b878055ee4ac31e78ca8188bb1f406557eb427816d5cdd023a0000000000000028aa8317bec2db715282ed641758a43cd899d6b144ea57670afaaa8b06058fadfd5d12ad5aef0c2da90fc8c4cab83796aaf7f940ce7b2c90b35fbc774c71724b4b507b6e6bba497f0e66cfb875e48f94df10669deae88277e3cb2e7e145fa38d10cc40a6a963885dcc465ef3ae33613aa31946b971990a8255ee9b7cef615d245df811781ddc2e69e1e05e1a39f6a2b0afeb6dcf823100d908ced0eef6dec65fd5838e55a169e5b721d5ea338a9c774a8b00000000000000201fd7137d18bc3d68dd74bf8b883288f9f69134af0f9a7678e89650f6eb0d00bcf1ee85af6b3aa34dc3abdf394e46eba02a4123be9e0b9cffe1f4cc94a849a72e866fdbee4f54ed6e390b7878bc32c9162a53cc700cc76f511553beb3e8a5548c18a465ddf34b54fa622d570c68df841d26d321c55313342ea9aeadffb01bd170061b9f3ef1247dc95939065a21bb13292ada858065e5f0136b2587206a1ea1e70ae9518be6ce63feffc2dc73a9a3dcca2c20e458708396b77610e843afbf370658a9265717eec815ec959c7f56d0a1af4ddbf7ae443576ef97165fdf3261f2c056d27fee3a97d486d276656fe3f7f7318089ca569fba710ac82e33ffcaeae4486605d1f70218a3c5b8f7807332e0855d7464f0014826d0e99760074844ddf19a0d7710553e8017dd706e272317ba8cda3eb5c07ec47d7a83914d8e643d02f648556a961eef4d03001b58b5fde1caca410ed780a6b0949d4edaace4f2ec336ec42f54c599e544b15d90eee64edbbc4099e1007b8a3be073eaba00a90bf614d63a4ec5817d88f1f96c0e1b0b557b180652b48a369a8734d35a8e88ad95d3abd6766de60217b7be22fd92b72aaec06c2a25a254ff6ef5673e96964d5221667f55f42e87900980a43ec4d757dd78a4c7af24309798b4a7ffa5ab66292082c58abf144d6319ceb1cf15d7110945e660734f4c9c14714a1356226fcde8c8c418546bca29696b815c3ef6ec1a41f52ecfa5df9ae36cda15ebc44b59381b01ff88ebe4b63f22ce019c4b7d7dcabe79a334aa8b17acce32a10a91202176d90d69184ea2d802a28417765a91ac60a05b68c8f37f3eb73695f9cd0c9d3add3e224e607c7ea60eb15a0f3b7b4576a27b7e166723e0daaf07f798977827bec92180dbf2a4f9f502f614797d1d817b9749dbb5bd5088aba720a3ce3fe985bcaab0834c859b2b02505d9876f1179619ec39edc8f48f6dadc66886e872b655a955ffebc6341ca394077aa28a6872b57bf154afa910ded2f1c2593765229ea01a60588ab939bf8adc14aac406df62a31a49f0356485340733422c1027ee253c52287267b361d040445a82e827d3d1082edac263c7034ca25481b162031b6bf6b4a1acdb84ed2108fe00000000000002a0e1efaa9f098cf6af927d2a03b6b6735e23725f60a6412123f4b7c08c42e8d8067a2e00fd8c3b3af67d2929e1f7c739a300000000000000010000000000000000e1efaa9f098cf6af927d2a03b6b6735e23725f60a6412123f4b7c08c42e8d8067a2e00fd8c3b3af67d2929e1f7c739a3";

/// Main error type for the crate.
#[derive(Error, Debug, Clone)]
pub enum Error {
    /// Error occurred when creating the Genesis DBC.
    #[error("Genesis DBC error:: {0}")]
    GenesisDbcError(String),
    /// The dbc error reason that parsing failed.
    #[error("Failed to parse reason: {0}")]
    FailedToParseReason(#[from] Box<DbcError>),
}

lazy_static! {
    /// Load the genesis DBC.
    /// The genesis DBC is the first DBC in the network. It is created without
    /// a source transaction, as there was nothing before it.
    pub static ref GENESIS_DBC: Dbc = match sn_dbc::Dbc::from_hex(GENESIS_DBC_HEX) {
        Ok(dbc) => dbc,
        Err(err) => panic!("Failed to read genesis DBC: {err:?}"),
    };
}

/// Return if provided DbcTransaction is genesis parent tx.
pub fn is_genesis_parent_tx(parent_tx: &DbcTransaction) -> bool {
    parent_tx == &GENESIS_DBC.src_tx
}

pub async fn load_genesis_wallet() -> LocalWallet {
    info!("Loading genesis...");
    let mut genesis_wallet = create_genesis_wallet().await;

    info!("Depositing genesis DBC: {:#?}", GENESIS_DBC.id());
    genesis_wallet.deposit(vec![GENESIS_DBC.clone()]);
    genesis_wallet
        .store()
        .await
        .expect("Genesis wallet shall be stored successfully.");

    let genesis_balance = genesis_wallet.balance();
    info!("Genesis wallet balance: {genesis_balance}");

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
#[cfg(test)]
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

pub async fn create_faucet_wallet() -> LocalWallet {
    let root_dir = get_faucet_dir().await;
    LocalWallet::load_from(&root_dir)
        .await
        .expect("Faucet wallet shall be created successfully.")
}

// We need deterministic and fix path for the genesis wallet.
// Otherwise the test instances will not be able to find the same genesis instance.
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
