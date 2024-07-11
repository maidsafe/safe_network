// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::error::Result;
use bip39::Mnemonic;
use sn_transfers::{get_faucet_data_dir, HotWallet, MainSecretKey};
use std::path::Path;

pub mod user_secret;

const DEFAULT_WALLET_DERIVIATION_PASSPHRASE: &str = "default";

/// Load a account from disk, with wallet, or create a new one using the mnemonic system
pub fn load_account_wallet_or_create_with_mnemonic(
    root_dir: &Path,
    derivation_passphrase: Option<&str>,
) -> Result<HotWallet> {
    let wallet = HotWallet::load_from(root_dir);

    match wallet {
        Ok(wallet) => Ok(wallet),
        Err(error) => {
            warn!("Issue loading wallet, creating a new one: {error}");
            println!("Issue loading wallet from {root_dir:?}");

            let mnemonic = load_or_create_mnemonic(root_dir)?;
            let wallet =
                secret_key_from_mnemonic(mnemonic, derivation_passphrase.map(|v| v.to_owned()))?;

            Ok(HotWallet::create_from_key(root_dir, wallet)?)
        }
    }
}

pub fn load_or_create_mnemonic(root_dir: &Path) -> Result<Mnemonic> {
    match user_secret::read_mnemonic_from_disk(root_dir) {
        Ok(mnemonic) => {
            println!(
                "Found existing mnemonic in {root_dir:?}, this will be used for key derivation."
            );
            info!("Using existing mnemonic from {root_dir:?}");
            Ok(mnemonic)
        }
        Err(error) => {
            println!("No existing mnemonic found, creating a new one in {root_dir:?}.");
            warn!("No existing mnemonic found in {root_dir:?}, creating new one. Error was: {error:?}");
            let mnemonic = user_secret::random_eip2333_mnemonic()?;
            user_secret::write_mnemonic_to_disk(root_dir, &mnemonic)?;
            Ok(mnemonic)
        }
    }
}

pub fn secret_key_from_mnemonic(
    mnemonic: Mnemonic,
    derivation_passphrase: Option<String>,
) -> Result<MainSecretKey> {
    let passphrase =
        derivation_passphrase.unwrap_or(DEFAULT_WALLET_DERIVIATION_PASSPHRASE.to_owned());
    user_secret::account_wallet_secret_key(mnemonic, &passphrase)
}

pub fn create_faucet_account_and_wallet() -> HotWallet {
    let root_dir = get_faucet_data_dir();

    println!("Loading faucet wallet... {root_dir:#?}");
    load_account_wallet_or_create_with_mnemonic(&root_dir, None)
        .expect("Faucet wallet shall be created successfully.")
}
