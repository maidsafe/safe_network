// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::wallet::encryption::EncryptedSecretKey;
use crate::wallet::{Error, Result};
use chrono::{DateTime, Duration, Utc};
use secrecy::{ExposeSecret, Secret};
use std::path::PathBuf;

/// Time (in seconds) before the user has to provide the password again for an encrypted wallet
const PASSWORD_EXPIRATION_TIME_SECS: i64 = 120;

/// Manager that makes it easier to interact with encrypted wallets
pub struct AuthenticationManager {
    // Password to decrypt the wallet.
    // Wrapped in Secret<> so that it doesn't accidentally get exposed
    password: Option<Secret<String>>,
    // Expiry time of the password.
    // Has to be provided by the user again after a certain amount of time
    password_expires_at: Option<DateTime<Utc>>,
    // Path to the root directory of the wallet
    wallet_dir: PathBuf,
}

impl AuthenticationManager {
    pub fn new(wallet_dir: PathBuf) -> Self {
        Self {
            password: None,
            password_expires_at: None,
            wallet_dir,
        }
    }

    pub fn set_password(&mut self, password: String) -> Result<()> {
        self.verify_password(&password)?;
        self.password = Some(Secret::new(password));
        self.reset_password_expiration_time();
        Ok(())
    }

    fn verify_password(&self, password: &str) -> Result<()> {
        let encrypted_secret_key = EncryptedSecretKey::from_file(self.wallet_dir.as_path())?;
        // Check if password is correct by trying to decrypt
        encrypted_secret_key.decrypt(password)?;
        Ok(())
    }

    fn reset_password_expiration_time(&mut self) {
        self.password_expires_at =
            Some(Utc::now() + Duration::seconds(PASSWORD_EXPIRATION_TIME_SECS));
    }

    /// Returns password if wallet is encrypted.
    /// Is None if wallet is not encrypted.
    /// Is Some if wallet is encrypted and password is available.
    /// Fails if wallet is encrypted, but no password available. In that case, the password needs to be set using `set_password()`.
    pub fn authenticate(&mut self) -> Result<Option<String>> {
        // If wallet is encrypted, require a valid password
        if EncryptedSecretKey::file_exists(self.wallet_dir.as_path()) {
            // Check if a password is set
            if let (Some(password), Some(expiration_time)) =
                (&self.password.to_owned(), self.password_expires_at)
            {
                // Check if password hasn't expired
                if Utc::now() <= expiration_time {
                    // Renew password expiration time after authenticating
                    self.reset_password_expiration_time();
                    Ok(Some(password.expose_secret().to_owned()))
                } else {
                    // Password is no longer active.
                    // Password needs to be set again using `set_password()`
                    Err(Error::WalletPasswordExpired)
                }
            } else {
                // Password needs to be set using `set_password()`
                Err(Error::WalletPasswordRequired)
            }
        } else {
            // Wallet is not encrypted
            Ok(None)
        }
    }
}
