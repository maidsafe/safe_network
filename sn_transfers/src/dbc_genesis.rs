// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::wallet::LocalWallet;

#[cfg(test)]
use sn_dbc::{
    rng, DbcIdSource, Hash, InputHistory, RevealedAmount, RevealedInput, TransactionBuilder,
};

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
const GENESIS_DBC_SK: &str = "0c5152498fc5b2f9ed691ef875f2c16f1f950910391f7ba1df63e9f0ce4b2780";
const GENESIS_DBC_HEX: &str = "1bc776bc3e819d47c12fc1ed39bf60018b4dc74f44c26d281baa1e89004a86559e04099af9dba798a7c9b4ae235b2f17c720ffd33947db0706097ff694f7518f9d10870a88068ade9b906d3e4899efb06c177791a6cc10c13a8368f490c8ec91c45d0fc46aabfabe494fde12be87499b18076db4a786f94216ebc7cb38a3da5118a465ddf34b54fa622d570c68df841d26d321c55313342ea9aeadffb01bd170b5933f7c1bb1ed8479fdda4e71a6f81c8254447995fb6beba1957e92c33a517718a465ddf34b54fa622d570c68df841d26d321c55313342ea9aeadffb01bd17009a04326ec82032317d76f31f657f6de05f616a5877fb9a776e1141f8fd5f83f009f1ab986e80aec08d9583d7d57432a6ee4c96d27418682312da4da821e57e92e0e62f72fd2a68788cbb3fccca7a132bc23981225a4da6957e8eb34a7b4f6641efb2e561d7a88325e5b228433f476f89d16dbbbf0e084b313113ff930bf671012071c24b396e8dad35101eea0e32fe0a9b0d29fc82e749cb79bb58550c4aa08101aa2aebab4303a14665021cb50becf50b235f32d603c0ec3289b97921f68a8202236f7a42faff2f05e5e608f92e29a9125ac2924102cf1e20470f1b74fd38a67b789c3015bfff37882c1453209ded26003ab1ea674c7359b7499f62338982005681d7ff992c38016ed08e30aacb0865dc25814c8dce8f341de871c367d36fa148ad8f45676a2d714c0bdaae2c047d902a95207afeb27ac9daf757c071a50003e3a52d1941e0914843627c028d91f2ab598ab4a8570e5902a9a836b85e048aa1514960fe231aba88124130bf54195c0d0deda5a7cb56abb1b68c073ffc58d623d5667ae8ea981642fb64ea4cd1d4f64b5f1a00df9c162ac9a5dabee87f1c5103f83af3e04bbeb371222a7a472397387e76e72944f645a0859c79004e1d904da04b6760252dc710168a67e8c43ce15cfd71bbc460f67bb15e43fb3da670ca3bb01cb133d1e49da4c3daf12b9b6692d3d776ca0d251f064a83d215b2c60c893e800e9775a92497e2b8ae6a4bb94da8a1c7cc3acaa492ed0c5d40a0011df334b4139f9dc4deb937e90ad0d535bdd7b334e02756de49890a4c4c80429965c9e3ac8344e6f84f1f326dedf91f1cf90a722cb16f4a81f05ca4515fb27d4eaf76fe5d82f175f9ab6733a3ede04cc0f9913afe836b006b4490e030bbbe4afb72e6078104a4624831937a32fe291a894d4d6a372d655c5a82116a38df751906789adc68000000000000002a0dd319adfeeb6cb46acdb6ee72aa7d0f82f3a0bab3f0409279f7289d4d35c431c2c8fa15fa24b703135308ecf72a71396000000000000000177a2315fc70b8072892cc6b32002606b9313f0955d2479e67ed79ee14cd8e6f3cb3b77f3d07f1028945eedfedea47204239096d8af7064a07fd1a98c6fb29e0ddf966a63837d37be71a3651ad385fb40ef8ac8d74ed524f0bf2367008062a79318a465ddf34b54fa622d570c68df841d26d321c55313342ea9aeadffb01bd170dd319adfeeb6cb46acdb6ee72aa7d0f82f3a0bab3f0409279f7289d4d35c431c2c8fa15fa24b703135308ecf72a713960000000000000001dd319adfeeb6cb46acdb6ee72aa7d0f82f3a0bab3f0409279f7289d4d35c431c2c8fa15fa24b703135308ecf72a71396000000000000000179b5c48903356ea75871f5bbc9ae93bdae6d357ceb1271b03e55aa7840d8a5934e1a8a14e609076e0f9082675738b10321110be53aad4b66b4633db936979146ca5f130ed5a5422b01361aab013888b646c4cb7e625fb01966bcf26967437784b3b90c1c5ab4cc30a8b5e253ba5eb3740ea2be93aacdb57a0ca77d67fe12ecaafd9268bd7ee0041f00000000000000282d552f54de3a886b354a5adae2a18a87fb49519623d0649df35a960810e6e5bed036b4fc06b3f0a5787e1ba2d3287dae1637c1989221a3306e773b826e5ec8c7975339e8ee3b6a948a47069580971dfea35a9d0a3f439ea865dd5d5b65581817f4c4ada8d9b1ddc08561d35b98ce6425b5526044019f0a3fb47ca6be42779e6181374bccd35806be4d0a9f83823b1cb601032cf778ef3c31d7be658779f47e56e3bb9efc48a85b0d6d1576c978b8e0bf00000000000000208bfd2f4eb0fb2c581aa8435df013e96f7726ec35378145e6ccd55fcd470c0bed39ec4c8d4c9e4aced32d23c88da71bae29707d296c62de5995b5bc1eeffab0e818ea21d0ddb8be92f94b8d3635100c44b97aba7c544609b6164cd15add3062a218a465ddf34b54fa622d570c68df841d26d321c55313342ea9aeadffb01bd17009a04326ec82032317d76f31f657f6de05f616a5877fb9a776e1141f8fd5f83f009f1ab986e80aec08d9583d7d57432a6ee4c96d27418682312da4da821e57e92e0e62f72fd2a68788cbb3fccca7a132bc23981225a4da6957e8eb34a7b4f6641efb2e561d7a88325e5b228433f476f89d16dbbbf0e084b313113ff930bf671012071c24b396e8dad35101eea0e32fe0a9b0d29fc82e749cb79bb58550c4aa08101aa2aebab4303a14665021cb50becf50b235f32d603c0ec3289b97921f68a8202236f7a42faff2f05e5e608f92e29a9125ac2924102cf1e20470f1b74fd38a67b789c3015bfff37882c1453209ded26003ab1ea674c7359b7499f62338982005681d7ff992c38016ed08e30aacb0865dc25814c8dce8f341de871c367d36fa148ad8f45676a2d714c0bdaae2c047d902a95207afeb27ac9daf757c071a50003e3a52d1941e0914843627c028d91f2ab598ab4a8570e5902a9a836b85e048aa1514960fe231aba88124130bf54195c0d0deda5a7cb56abb1b68c073ffc58d623d5667ae8ea981642fb64ea4cd1d4f64b5f1a00df9c162ac9a5dabee87f1c5103f83af3e04bbeb371222a7a472397387e76e72944f645a0859c79004e1d904da04b6760252dc710168a67e8c43ce15cfd71bbc460f67bb15e43fb3da670ca3bb01cb133d1e49da4c3daf12b9b6692d3d776ca0d251f064a83d215b2c60c893e800e9775a92497e2b8ae6a4bb94da8a1c7cc3acaa492ed0c5d40a0011df334b4139f9dc4deb937e90ad0d535bdd7b334e02756de49890a4c4c80429965c9e3ac8344e6f84f1f326dedf91f1cf90a722cb16f4a81f05ca4515fb27d4eaf76fe5d82f175f9ab6733a3ede04cc0f9913afe836b006b4490e030bbbe4afb72e6078104a4624831937a32fe291a894d4d6a372d655c5a82116a38df751906789adc68000000000000002a0dd319adfeeb6cb46acdb6ee72aa7d0f82f3a0bab3f0409279f7289d4d35c431c2c8fa15fa24b703135308ecf72a71396000000000000000177a2315fc70b8072892cc6b32002606b9313f0955d2479e67ed79ee14cd8e6f3cb3b77f3d07f1028945eedfedea47204239096d8af7064a07fd1a98c6fb29e0ddf966a63837d37be71a3651ad385fb40ef8ac8d74ed524f0bf2367008062a79318a465ddf34b54fa622d570c68df841d26d321c55313342ea9aeadffb01bd170dd319adfeeb6cb46acdb6ee72aa7d0f82f3a0bab3f0409279f7289d4d35c431c2c8fa15fa24b703135308ecf72a713960000000000000001dd319adfeeb6cb46acdb6ee72aa7d0f82f3a0bab3f0409279f7289d4d35c431c2c8fa15fa24b703135308ecf72a71396";

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
    println!("Loading genesis...");
    let mut genesis_wallet = create_genesis_wallet().await;
    let genesis_balance = genesis_wallet.balance();
    if genesis_balance.as_nano() > 0 {
        println!("Genesis wallet balance: {genesis_balance}");
        return genesis_wallet;
    }

    use super::wallet::{DepositWallet, Wallet};
    genesis_wallet.deposit(vec![GENESIS_DBC.clone()]);
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
#[cfg(test)]
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
        .add_output(sn_dbc::Token::from_nano(GENESIS_DBC_AMOUNT), dbc_id_src)
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

    let derived_key = dbc
        .derived_key(main_key)
        .map_err(|e| Error::FailedToParseReason(Box::new(e)))?;
    let revealed_amount = dbc
        .revealed_amount(&derived_key)
        .map_err(|e| Error::FailedToParseReason(Box::new(e)))?;
    let input = InputHistory {
        input: RevealedInput::new(derived_key, revealed_amount),
        input_src_tx: dbc.src_tx.clone(),
    };

    let recipients: Vec<_> = (0..number)
        .map(|_| {
            let dbc_id_src = main_key.random_dbc_id_src(rng);
            let amount = revealed_amount.value() / number as u64;
            (sn_dbc::Token::from_nano(amount), dbc_id_src)
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
