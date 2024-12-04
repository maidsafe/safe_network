//! Network-related functionality and types

pub mod error;
pub mod record;
pub mod types;

pub use error::{GetRecordError, NetworkError};
pub use record::{GetRecordCfg, PutRecordCfg, VerificationKind};
pub use types::PayeeQuote;
