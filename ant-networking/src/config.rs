//! Configuration constants and settings for the networking module.
use std::time::Duration;

/// Maximum allowed size for network packets in bytes (2MB)
pub const MAX_PACKET_SIZE: usize = 2 * 1024 * 1024;

/// Number of nodes to maintain in the close group
pub const CLOSE_GROUP_SIZE: usize = 8;

/// Default timeout duration for network requests in seconds
pub const REQUEST_TIMEOUT_DEFAULT_S: u64 = 60;

/// Duration to keep connections alive
pub const CONNECTION_KEEP_ALIVE_TIMEOUT: Duration = Duration::from_secs(20);

/// Timeout duration for Kademlia queries in seconds
pub const KAD_QUERY_TIMEOUT_S: u64 = 20;

/// Protocol identifier for Kademlia streams
pub const KAD_STREAM_PROTOCOL_ID: &str = "/safe/kad/1.0.0";

/// Size of the networking channel buffer
pub const NETWORKING_CHANNEL_SIZE: usize = 100;

/// Interval for relay manager reservation checks
pub const RELAY_MANAGER_RESERVATION_INTERVAL: Duration = Duration::from_secs(30);

/// Interval for resending identify messages
pub const RESEND_IDENTIFY_INVERVAL: Duration = Duration::from_secs(300);

/// Configuration for the networking component
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    pub max_packet_size: usize,
    pub close_group_size: usize,
    pub request_timeout: Duration,
    pub connection_keep_alive: Duration,
    pub kad_query_timeout: Duration,
    pub channel_size: usize,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            max_packet_size: MAX_PACKET_SIZE,
            close_group_size: CLOSE_GROUP_SIZE,
            request_timeout: Duration::from_secs(REQUEST_TIMEOUT_DEFAULT_S),
            connection_keep_alive: CONNECTION_KEEP_ALIVE_TIMEOUT,
            kad_query_timeout: Duration::from_secs(KAD_QUERY_TIMEOUT_S),
            channel_size: NETWORKING_CHANNEL_SIZE,
        }
    }
} 