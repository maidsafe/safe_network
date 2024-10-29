#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Private key is invalid")]
    InvalidPrivateKey,
    #[error("Private key file is invalid")]
    InvalidPrivateKeyFile,
    #[error("Failed to encrypt private key: {0}")]
    FailedToEncryptKey(String),
    #[error("Failed to decrypt private key: {0}")]
    FailedToDecryptKey(String),
    #[error("Failed to write private key to disk: {0}")]
    FailedToStorePrivateKey(String),
    #[error("Failed to find wallets folder")]
    WalletsFolderNotFound,
    #[error("Failed to create wallets folder")]
    FailedToCreateWalletsFolder,
    #[error("Could not find private key file")]
    PrivateKeyFileNotFound,
    #[error("No wallets found. Create one using `wallet create`")]
    NoWalletsFound,
    #[error("Invalid wallet selection input")]
    InvalidSelection,
}
