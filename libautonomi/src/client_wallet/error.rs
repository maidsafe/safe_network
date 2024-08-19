use sn_transfers::UniquePubkey;
use std::collections::BTreeSet;

#[derive(Debug, thiserror::Error)]
pub enum SendSpendsError {
    /// The cashnotes that were attempted to be spent have already been spent to another address
    #[error("Double spend attempted with cashnotes: {0:?}")]
    DoubleSpendAttemptedForCashNotes(BTreeSet<UniquePubkey>),
    /// A general error when a transfer fails
    #[error("Failed to send tokens due to {0}")]
    CouldNotSendMoney(String),
}

#[derive(Debug, thiserror::Error)]
pub enum TransferError {
    #[error("Failed to send tokens due to {0}")]
    CouldNotSendMoney(String),
    #[error("Wallet error: {0:?}")]
    WalletError(#[from] crate::wallet::error::WalletError),
    #[error("Network error: {0:?}")]
    NetworkError(#[from] sn_client::networking::NetworkError),
}

#[derive(Debug, thiserror::Error)]
pub enum CashNoteError {
    #[error("CashNote was already spent.")]
    AlreadySpent,
    #[error("Failed to get spend: {0:?}")]
    FailedToGetSpend(String),
}
