use alloy::transports::{RpcError, TransportErrorKind};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    ContractError(#[from] alloy::contract::Error),
    #[error(transparent)]
    RpcError(#[from] RpcError<TransportErrorKind>),
    #[error(transparent)]
    PendingTransactionError(#[from] alloy::providers::PendingTransactionError),
    #[error("Payment is invalid.")]
    PaymentInvalid,
    #[error("Payment verification length must be 3.")]
    PaymentVerificationLengthInvalid,
}
