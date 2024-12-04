//! Configuration constants and settings for the networking module.
use std::time::Duration;
use thiserror::Error;

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

/// Errors that can occur during network configuration
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Packet size {0} exceeds maximum allowed size of {MAX_PACKET_SIZE}")]
    PacketSizeTooLarge(usize),
    #[error("Close group size {0} is invalid (must be between 3 and 20)")]
    InvalidCloseGroupSize(usize),
    #[error("Timeout {0:?} is too short (minimum {1:?})")]
    TimeoutTooShort(Duration, Duration),
    #[error("Channel size {0} is too small (minimum 10)")]
    ChannelSizeTooSmall(usize),
}

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

impl NetworkConfig {
    /// Creates a new NetworkConfigBuilder
    pub fn builder() -> NetworkConfigBuilder {
        NetworkConfigBuilder::default()
    }

    /// Validates the configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.max_packet_size > MAX_PACKET_SIZE {
            return Err(ConfigError::PacketSizeTooLarge(self.max_packet_size));
        }

        if self.close_group_size < 3 || self.close_group_size > 20 {
            return Err(ConfigError::InvalidCloseGroupSize(self.close_group_size));
        }

        let min_timeout = Duration::from_secs(10);
        if self.request_timeout < min_timeout {
            return Err(ConfigError::TimeoutTooShort(self.request_timeout, min_timeout));
        }

        if self.channel_size < 10 {
            return Err(ConfigError::ChannelSizeTooSmall(self.channel_size));
        }

        Ok(())
    }
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

/// Builder for NetworkConfig
#[derive(Debug, Default)]
pub struct NetworkConfigBuilder {
    max_packet_size: Option<usize>,
    close_group_size: Option<usize>,
    request_timeout: Option<Duration>,
    connection_keep_alive: Option<Duration>,
    kad_query_timeout: Option<Duration>,
    channel_size: Option<usize>,
}

impl NetworkConfigBuilder {
    pub fn max_packet_size(mut self, size: usize) -> Self {
        self.max_packet_size = Some(size);
        self
    }

    pub fn close_group_size(mut self, size: usize) -> Self {
        self.close_group_size = Some(size);
        self
    }

    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = Some(timeout);
        self
    }

    pub fn connection_keep_alive(mut self, timeout: Duration) -> Self {
        self.connection_keep_alive = Some(timeout);
        self
    }

    pub fn kad_query_timeout(mut self, timeout: Duration) -> Self {
        self.kad_query_timeout = Some(timeout);
        self
    }

    pub fn channel_size(mut self, size: usize) -> Self {
        self.channel_size = Some(size);
        self
    }

    pub fn build(self) -> Result<NetworkConfig, ConfigError> {
        let config = NetworkConfig {
            max_packet_size: self.max_packet_size.unwrap_or(MAX_PACKET_SIZE),
            close_group_size: self.close_group_size.unwrap_or(CLOSE_GROUP_SIZE),
            request_timeout: self.request_timeout.unwrap_or(Duration::from_secs(REQUEST_TIMEOUT_DEFAULT_S)),
            connection_keep_alive: self.connection_keep_alive.unwrap_or(CONNECTION_KEEP_ALIVE_TIMEOUT),
            kad_query_timeout: self.kad_query_timeout.unwrap_or(Duration::from_secs(KAD_QUERY_TIMEOUT_S)),
            channel_size: self.channel_size.unwrap_or(NETWORKING_CHANNEL_SIZE),
        };

        config.validate()?;
        Ok(config)
    }
} 