pub mod config;
pub mod network;
pub mod types;

#[cfg(test)]
mod tests;

// Re-exports
pub use config::NetworkConfig;
pub use crate::network::{
    error::{GetRecordError, NetworkError},
    record::{GetRecordCfg, PutRecordCfg, VerificationKind},
    types::PayeeQuote,
};
pub use types::{NetworkAddress, NetworkMetricsRecorder, NodeIssue};

// Utility functions
use libp2p::Multiaddr;

/// Checks if a multiaddress is globally reachable
pub fn multiaddr_is_global(_addr: &Multiaddr) -> bool {
    // Implementation here...
    true // Placeholder
}

/// Re-export tokio spawn for convenience
pub use tokio::spawn;

/// Re-export tokio time utilities
pub mod target_arch {
    pub use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
    pub use tokio::spawn;
}
