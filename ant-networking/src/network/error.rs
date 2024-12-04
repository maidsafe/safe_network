use crate::config::ConfigError;
use libp2p::{kad, PeerId};
use std::time::Duration;
use thiserror::Error;

/// Error type for get record operations
#[derive(Debug, Error)]
pub enum GetRecordError {
    /// Record not found
    #[error("Record not found")]
    NotFound,
    /// Verification failed
    #[error("Verification failed: {0}")]
    VerificationFailed(String),
    /// Network error occurred
    #[error("Network error: {0}")]
    Network(#[from] NetworkError),
}

/// Comprehensive error type for network operations
#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("Connection to peer {peer_id} failed: {reason}")]
    Connection {
        peer_id: PeerId,
        reason: String,
    },

    #[error("Record operation failed: {0}")]
    Record(#[from] RecordError),

    #[error("Kademlia operation failed: {0}")]
    Kademlia(String),

    #[error("Operation timed out after {0:?}")]
    Timeout(Duration),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Other error: {0}")]
    Other(String),
}

/// Errors specific to record operations
#[derive(Debug, Error)]
pub enum RecordError {
    #[error("Record not found")]
    NotFound,

    #[error("Record verification failed: {0}")]
    VerificationFailed(String),

    #[error("Record size {size} exceeds maximum allowed size {max_size}")]
    SizeExceeded {
        size: usize,
        max_size: usize,
    },

    #[error("Record expired at {0:?}")]
    Expired(std::time::SystemTime),

    #[error("Invalid record format: {0}")]
    InvalidFormat(String),

    #[error("Storage error: {0}")]
    Storage(String),
}

/// Result type for network operations
pub type Result<T> = std::result::Result<T, NetworkError>;

/// Result type for record operations
pub type RecordResult<T> = std::result::Result<T, RecordError>;

impl From<RecordError> for GetRecordError {
    fn from(err: RecordError) -> Self {
        match err {
            RecordError::NotFound => GetRecordError::NotFound,
            RecordError::VerificationFailed(msg) => GetRecordError::VerificationFailed(msg),
            err => GetRecordError::Network(NetworkError::Record(err)),
        }
    }
}
