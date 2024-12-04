use crate::config::{
    ConfigError, NetworkConfig, 
    CLOSE_GROUP_SIZE, CONNECTION_KEEP_ALIVE_TIMEOUT, KAD_QUERY_TIMEOUT_S, MAX_PACKET_SIZE,
    NETWORKING_CHANNEL_SIZE, REQUEST_TIMEOUT_DEFAULT_S,
};
use std::time::Duration;

#[test]
fn test_network_config_builder() {
    let config = NetworkConfig::builder()
        .max_packet_size(1024)
        .close_group_size(4)
        .request_timeout(Duration::from_secs(30))
        .connection_keep_alive(Duration::from_secs(10))
        .kad_query_timeout(Duration::from_secs(15))
        .channel_size(50)
        .build()
        .unwrap();

    assert_eq!(config.max_packet_size, 1024);
    assert_eq!(config.close_group_size, 4);
    assert_eq!(config.request_timeout, Duration::from_secs(30));
    assert_eq!(config.connection_keep_alive, Duration::from_secs(10));
    assert_eq!(config.kad_query_timeout, Duration::from_secs(15));
    assert_eq!(config.channel_size, 50);
}

#[test]
fn test_network_config_validation() {
    // Test invalid packet size
    let result = NetworkConfig::builder()
        .max_packet_size(MAX_PACKET_SIZE + 1)
        .build();
    assert!(matches!(result, Err(ConfigError::PacketSizeTooLarge(_))));

    // Test invalid close group size
    let result = NetworkConfig::builder()
        .close_group_size(2)
        .build();
    assert!(matches!(result, Err(ConfigError::InvalidCloseGroupSize(_))));

    // Test invalid timeout
    let result = NetworkConfig::builder()
        .request_timeout(Duration::from_secs(5))
        .build();
    assert!(matches!(result, Err(ConfigError::TimeoutTooShort(_, _))));

    // Test invalid channel size
    let result = NetworkConfig::builder()
        .channel_size(5)
        .build();
    assert!(matches!(result, Err(ConfigError::ChannelSizeTooSmall(_))));
}

#[test]
fn test_network_config_defaults() {
    let config = NetworkConfig::default();
    
    assert_eq!(config.max_packet_size, MAX_PACKET_SIZE);
    assert_eq!(config.close_group_size, CLOSE_GROUP_SIZE);
    assert_eq!(config.request_timeout, Duration::from_secs(REQUEST_TIMEOUT_DEFAULT_S));
    assert_eq!(config.connection_keep_alive, CONNECTION_KEEP_ALIVE_TIMEOUT);
    assert_eq!(config.kad_query_timeout, Duration::from_secs(KAD_QUERY_TIMEOUT_S));
    assert_eq!(config.channel_size, NETWORKING_CHANNEL_SIZE);
} 