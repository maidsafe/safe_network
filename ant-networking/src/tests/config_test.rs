use crate::config::{
    CLOSE_GROUP_SIZE, CONNECTION_KEEP_ALIVE_TIMEOUT, KAD_QUERY_TIMEOUT_S, MAX_PACKET_SIZE,
    NETWORKING_CHANNEL_SIZE, NetworkConfig, REQUEST_TIMEOUT_DEFAULT_S,
    RELAY_MANAGER_RESERVATION_INTERVAL, RESEND_IDENTIFY_INVERVAL,
};
use std::time::Duration;

#[test]
fn test_network_config_creation() {
    let custom_config = NetworkConfig {
        max_packet_size: 1024,
        close_group_size: 4,
        request_timeout: Duration::from_secs(30),
        connection_keep_alive: Duration::from_secs(10),
        kad_query_timeout: Duration::from_secs(15),
        channel_size: 50,
    };

    assert_eq!(custom_config.max_packet_size, 1024);
    assert_eq!(custom_config.close_group_size, 4);
    assert_eq!(custom_config.request_timeout, Duration::from_secs(30));
    assert_eq!(custom_config.connection_keep_alive, Duration::from_secs(10));
    assert_eq!(custom_config.kad_query_timeout, Duration::from_secs(15));
    assert_eq!(custom_config.channel_size, 50);
}

#[test]
fn test_config_constants() {
    assert!(MAX_PACKET_SIZE > 0);
    assert!(CLOSE_GROUP_SIZE > 0);
    assert!(REQUEST_TIMEOUT_DEFAULT_S > 0);
    assert!(CONNECTION_KEEP_ALIVE_TIMEOUT > Duration::from_secs(0));
    assert!(KAD_QUERY_TIMEOUT_S > 0);
    assert!(NETWORKING_CHANNEL_SIZE > 0);
    assert!(RELAY_MANAGER_RESERVATION_INTERVAL > Duration::from_secs(0));
    assert!(RESEND_IDENTIFY_INVERVAL > Duration::from_secs(0));
}

#[test]
fn test_config_reasonable_values() {
    // Test that packet size is reasonable (not too small or large)
    assert!(MAX_PACKET_SIZE >= 1024); // At least 1KB
    assert!(MAX_PACKET_SIZE <= 1024 * 1024 * 10); // Not more than 10MB

    // Test that timeouts are reasonable
    assert!(REQUEST_TIMEOUT_DEFAULT_S >= 10); // At least 10 seconds
    assert!(REQUEST_TIMEOUT_DEFAULT_S <= 300); // Not more than 5 minutes

    // Test that group size is reasonable
    assert!(CLOSE_GROUP_SIZE >= 3); // At least 3 nodes for redundancy
    assert!(CLOSE_GROUP_SIZE <= 20); // Not too many nodes
} 