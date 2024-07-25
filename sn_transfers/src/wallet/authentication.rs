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
    /// Password to decrypt the wallet.
    /// Wrapped in Secret<> so that it doesn't accidentally get exposed
    password: Option<Secret<String>>,
    /// Expiry time of the password.
    /// Has to be provided by the user again after a certain amount of time
    password_expires_at: Option<DateTime<Utc>>,
    /// Path to the root directory of the wallet
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

    /// Authenticates the wallet using the provided password.
    /// Password will be saved (available) for a limited amount of time.
    pub fn authenticate_with_password(&mut self, password: String) -> Result<()> {
        self.verify_password(&password)?;
        self.password = Some(Secret::new(password));
        self.reset_password_expiration_time();
        Ok(())
    }

    /// Verifies the provided password against the encrypted secret key.
    fn verify_password(&self, password: &str) -> Result<()> {
        let encrypted_secret_key = EncryptedSecretKey::from_file(self.wallet_dir.as_path())?;
        // Check if password is correct by trying to decrypt
        encrypted_secret_key.decrypt(password)?;
        Ok(())
    }

    /// Resets the password expiration time to the current time plus the expiration duration.
    fn reset_password_expiration_time(&mut self) {
        self.password_expires_at =
            Some(Utc::now() + Duration::seconds(PASSWORD_EXPIRATION_TIME_SECS));
    }

    /// Authenticates the wallet and returns the password if it is encrypted.
    ///
    /// # Returns
    /// - `Ok(Some(String))`: The wallet is encrypted and the password is available and valid.
    /// - `Ok(None)`: The wallet is not encrypted.
    /// - `Err(Error)`: The wallet is encrypted, but no valid password is available.
    ///
    /// # Errors
    /// Returns an error in the following cases:
    /// - `Error::WalletPasswordExpired`: The wallet's password has expired and the user needs to authenticate again with a valid password using `authenticate_with_password()`.
    /// - `Error::WalletPasswordRequired`: The wallet is encrypted but no password is set. The user needs to authenticate with a valid password using `authenticate_with_password()`.
    pub fn authenticate(&mut self) -> Result<Option<String>> {
        // If wallet is encrypted, require a valid password
        if EncryptedSecretKey::file_exists(self.wallet_dir.as_path()) {
            // Check if a password is set
            if let (Some(password), Some(expiration_time)) =
                (&self.password.to_owned(), self.password_expires_at)
            {
                let password = password.expose_secret().to_owned();

                // Verify if password is still correct
                if self.verify_password(&password).is_err() {
                    self.password = None;
                    return Err(Error::WalletPasswordIncorrect);
                }

                // Check if password hasn't expired
                if Utc::now() <= expiration_time {
                    // Renew password expiration time after authenticating
                    self.reset_password_expiration_time();
                    Ok(Some(password))
                } else {
                    // Password is no longer active.
                    // User needs to authenticate again with a valid password
                    self.password = None;
                    Err(Error::WalletPasswordExpired)
                }
            } else {
                // User needs to authenticate with a valid password
                Err(Error::WalletPasswordRequired)
            }
        } else {
            // Wallet is not encrypted
            Ok(None)
        }
    }
}
