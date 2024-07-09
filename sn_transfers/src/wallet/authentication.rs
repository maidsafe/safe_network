use crate::wallet::Error;
use chrono::{DateTime, Duration, Utc};
use secrecy::{ExposeSecret, Secret};

/// Time before the user has to provide the password again
const PASSWORD_EXPIRATION_TIME_SECS: i64 = 120;

/// Manager that makes it easier to interact with encrypted wallets
#[derive(Default)]
pub struct AuthenticationManager {
    // Password to decrypt the wallet.
    // Wrapped in Secret<> so that it doesn't accidentally get exposed
    password: Option<Secret<String>>,
    // Expiry time of the password.
    // Has to be provided by the user again after a certain amount of time
    password_expires_at: Option<DateTime<Utc>>,
}

impl AuthenticationManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_password(&self) -> crate::wallet::Result<String> {
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

    pub fn set_password(&mut self, password: String) {
        self.password = Some(Secret::new(password));
        self.password_expires_at =
            Some(Utc::now() + Duration::seconds(PASSWORD_EXPIRATION_TIME_SECS));
    }
}
