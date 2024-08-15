use sn_client::acc_packet::user_secret::account_wallet_secret_key;
use sn_client::transfers::MainSecretKey;

#[derive(Debug, thiserror::Error)]
pub enum SecretsError {
    /// Should never happen
    #[error("Unexpected error")]
    Unexpected,
    /// Failed to parse entropy
    #[error("Error parsing entropy for mnemonic phrase")]
    FailedToParseEntropy,
    /// Invalid mnemonic seed phrase
    #[error("Invalid mnemonic seed phrase")]
    InvalidMnemonicSeedPhrase,
    /// Invalid key bytes
    #[error("Invalid key bytes")]
    InvalidKeyBytes,
}

impl From<sn_client::Error> for SecretsError {
    fn from(value: sn_client::Error) -> Self {
        match value {
            sn_client::Error::FailedToParseEntropy => SecretsError::FailedToParseEntropy,
            sn_client::Error::InvalidMnemonicSeedPhrase => SecretsError::InvalidMnemonicSeedPhrase,
            sn_client::Error::InvalidKeyBytes => SecretsError::InvalidKeyBytes,
            _ => SecretsError::Unexpected,
        }
    }
}

pub fn generate_mnemonic() -> Result<bip39::Mnemonic, SecretsError> {
    sn_client::acc_packet::user_secret::random_eip2333_mnemonic().map_err(SecretsError::from)
}

pub fn main_sk_from_mnemonic(
    mnemonic: bip39::Mnemonic,
    derivation_passphrase: &str,
) -> Result<MainSecretKey, SecretsError> {
    account_wallet_secret_key(mnemonic, derivation_passphrase).map_err(SecretsError::from)
}
