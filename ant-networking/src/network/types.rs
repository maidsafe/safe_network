use libp2p::PeerId;
use std::time::Duration;

/// Quote information for network payments
#[derive(Debug, Clone)]
pub struct PayeeQuote {
    pub peer_id: PeerId,
    pub price: u64,
    pub expiry: Duration,
} 