#[derive(Debug, thiserror::Error)]
pub enum WalletError {
    /// Happens when a wallet is trying to decrypt a transfer that was meant for another wallet.
    #[error("Failed to decrypt transfer with our key, maybe it was not for us")]
    FailedToDecryptTransfer,
    /// Error when attempting to transfer 0 tokens
    #[error("The transfer amount must be more than 0")]
    TransferAmountZero,
    #[error("Could not get value of `CashNote` as no outputs are found.")]
    CashNoteOutputNotFound,
    #[error("This `CashNote` is not destined for this wallet.")]
    CashNoteNotOwned,
    #[error(transparent)]
    TransferError(#[from] sn_transfers::TransferError),
}
