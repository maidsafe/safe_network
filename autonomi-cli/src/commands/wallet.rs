// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::wallet::fs::{get_client_wallet_dir_path, store_private_key};
use crate::wallet::input::request_password;
use crate::wallet::DUMMY_NETWORK;
use autonomi::Wallet;
use color_eyre::eyre::eyre;
use color_eyre::Result;

const WALLET_PASSWORD_REQUIRED: bool = false;

pub fn create(
    no_password: bool,
    private_key: Option<String>,
    password: Option<String>,
) -> Result<()> {
    if no_password && password.is_some() {
        return Err(eyre!(
            "Only one of `--no-password` or `--password` may be specified"
        ));
    }

    // Set a password for encryption or not
    let encryption_password: Option<String> = match (no_password, password) {
        (true, _) => None,
        (false, Some(pass)) => Some(pass.to_owned()),
        (false, None) => request_password(WALLET_PASSWORD_REQUIRED),
    };

    let wallet_private_key = if let Some(private_key) = private_key {
        // Validate imported key
        Wallet::new_from_private_key(DUMMY_NETWORK, &private_key)
            .map_err(|_| eyre!("Please provide a valid secret key in hex format"))?;

        private_key
    } else {
        // Create a new key
        Wallet::random_private_key()
    };

    let wallet_address = Wallet::new_from_private_key(DUMMY_NETWORK, &wallet_private_key)
        .expect("Infallible")
        .address()
        .to_string();

    // Save the private key file
    let file_path = store_private_key(&wallet_private_key, encryption_password)?;

    println!("Wallet address: {wallet_address}");
    println!("Stored wallet in: {file_path:?}");

    Ok(())
}

pub fn balance() -> Result<()> {
    Ok(())
}
