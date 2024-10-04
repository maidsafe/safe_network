use crate::contract::network_token;
use alloy::transports::{RpcError, TransportErrorKind};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    ContractError(#[from] alloy::contract::Error),
    #[error(transparent)]
    RpcError(#[from] RpcError<TransportErrorKind>),
    #[error(transparent)]
    NetworkTokenError(#[from] network_token::Error),
    #[error(transparent)]
    PendingTransactionError(#[from] alloy::providers::PendingTransactionError),
    #[error("The transfer limit of 256 has been exceeded")]
    TransferLimitExceeded,
}
