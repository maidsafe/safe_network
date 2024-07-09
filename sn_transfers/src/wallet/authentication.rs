use crate::wallet::encryption::EncryptedSecretKey;
use crate::wallet::{Error, Result};
use chrono::{DateTime, Duration, Utc};
use secrecy::{ExposeSecret, Secret};
use std::path::PathBuf;

/// Time before the user has to provide the password again
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

    pub fn get_password(&self) -> Result<String> {
        if let (Some(password), Some(expiration_time)) = (&self.password, self.password_expires_at)
        {
            if Utc::now() <= expiration_time {
                Ok(password.expose_secret().to_owned())
            } else {
                Err(Error::HotWalletPasswordExpired)
            }
        } else {
            Err(Error::HotWalletPasswordRequired)
        }
    }

    pub fn set_password(&mut self, password: String) -> Result<()> {
        let encrypted_secret_key = EncryptedSecretKey::from_file(self.wallet_dir.as_path())?;

        // Check if password is correct
        encrypted_secret_key.decrypt(&password)?;

        self.password = Some(Secret::new(password));
        self.password_expires_at =
            Some(Utc::now() + Duration::seconds(PASSWORD_EXPIRATION_TIME_SECS));

        Ok(())
    }

    /// Returns password if wallet is encrypted.
    /// Is None if wallet is not encrypted.
    /// Is Some if wallet is encrypted and password is available.
    /// Fails if wallet is encrypted, but no password available. In that case, the password needs to be set using `set_password()`.
    pub fn authenticate(&self) -> Result<Option<String>> {
        if EncryptedSecretKey::file_exists(self.wallet_dir.as_path()) {
            self.get_password().map(Some)
        } else {
            Ok(None)
        }
    }
}
