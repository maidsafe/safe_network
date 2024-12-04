use std::error::Error as StdError;
use std::fmt;

/// Represents errors that can occur during network operations
#[derive(Debug)]
pub enum NetworkError {
    /// Error occurred during record operations
    Record(String),
    /// Error occurred during connection operations
    Connection(String),
    /// General network error
    Other(String),
}

impl fmt::Display for NetworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkError::Record(msg) => write!(f, "Record error: {}", msg),
            NetworkError::Connection(msg) => write!(f, "Connection error: {}", msg),
            NetworkError::Other(msg) => write!(f, "Network error: {}", msg),
        }
    }
}

impl StdError for NetworkError {}

/// Error that can occur when getting records
#[derive(Debug)]
pub enum GetRecordError {
    /// Record not found
    NotFound,
    /// Verification failed
    VerificationFailed(String),
    /// Network error occurred
    Network(NetworkError),
}

impl fmt::Display for GetRecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GetRecordError::NotFound => write!(f, "Record not found"),
            GetRecordError::VerificationFailed(msg) => write!(f, "Verification failed: {}", msg),
            GetRecordError::Network(err) => write!(f, "Network error: {}", err),
        }
    }
}

impl StdError for GetRecordError {}
