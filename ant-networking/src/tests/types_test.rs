use crate::{
    types::{NetworkAddress, NetworkMetricsRecorder, NodeIssue},
    config::{
        CLOSE_GROUP_SIZE, CONNECTION_KEEP_ALIVE_TIMEOUT, KAD_QUERY_TIMEOUT_S, MAX_PACKET_SIZE,
        NETWORKING_CHANNEL_SIZE, NetworkConfig, REQUEST_TIMEOUT_DEFAULT_S,
    },
};
use libp2p::PeerId;
use std::{sync::Mutex, time::Duration};

#[test]
fn test_network_address_creation() {
    let peer_id = PeerId::random();
    let distance = 42;
    let addr = NetworkAddress::new(peer_id, distance);

    assert_eq!(addr.peer_id, peer_id);
    assert_eq!(addr.distance, distance);
    assert_eq!(addr.holder_count(), 0);
}

#[test]
fn test_network_address_holders() {
    let mut addr = NetworkAddress::new(PeerId::random(), 42);
    let holder1 = PeerId::random();
    let holder2 = PeerId::random();

    addr.add_holder(holder1);
    assert_eq!(addr.holder_count(), 1);
    assert!(addr.holders.contains(&holder1));

    // Adding same holder twice should not increase count
    addr.add_holder(holder1);
    assert_eq!(addr.holder_count(), 1);

    addr.add_holder(holder2);
    assert_eq!(addr.holder_count(), 2);
    assert!(addr.holders.contains(&holder2));
}

#[test]
fn test_node_issue_display() {
    let issues = vec![
        NodeIssue::ConnectionFailed("timeout".into()),
        NodeIssue::RecordStoreFailed("disk full".into()),
        NodeIssue::NetworkError("connection reset".into()),
    ];

    for issue in issues {
        let display_string = format!("{}", issue);
        assert!(!display_string.is_empty());
        match issue {
            NodeIssue::ConnectionFailed(_) => assert!(display_string.contains("Connection failed")),
            NodeIssue::RecordStoreFailed(_) => assert!(display_string.contains("Record store failed")),
            NodeIssue::NetworkError(_) => assert!(display_string.contains("Network error")),
        }
    }
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

#[test]
fn test_network_address_empty_holders() {
    let addr = NetworkAddress::new(PeerId::random(), 0);
    assert!(addr.holders.is_empty());
    assert_eq!(addr.holder_count(), 0);
}

#[test]
fn test_network_address_multiple_holders() {
    let mut addr = NetworkAddress::new(PeerId::random(), 10);
    let holders: Vec<PeerId> = (0..5).map(|_| PeerId::random()).collect();
    
    for holder in &holders {
        addr.add_holder(*holder);
    }
    
    assert_eq!(addr.holder_count(), 5);
    for holder in holders {
        assert!(addr.holders.contains(&holder));
    }
}

#[test]
fn test_node_issue_details() {
    let error_msg = "test error message";
    
    let connection_issue = NodeIssue::ConnectionFailed(error_msg.into());
    let record_issue = NodeIssue::RecordStoreFailed(error_msg.into());
    let network_issue = NodeIssue::NetworkError(error_msg.into());
    
    assert!(format!("{}", connection_issue).contains(error_msg));
    assert!(format!("{}", record_issue).contains(error_msg));
    assert!(format!("{}", network_issue).contains(error_msg));
}

// Add a mock implementation of NetworkMetricsRecorder for testing
#[derive(Default)]
struct MockMetricsRecorder {
    close_group_size: Mutex<usize>,
    connection_count: Mutex<usize>,
    record_store_size: Mutex<usize>,
    last_issue: Mutex<Option<NodeIssue>>,
}

impl NetworkMetricsRecorder for MockMetricsRecorder {
    fn record_close_group_size(&self, size: usize) {
        *self.close_group_size.lock().unwrap() = size;
    }

    fn record_connection_count(&self, count: usize) {
        *self.connection_count.lock().unwrap() = count;
    }

    fn record_record_store_size(&self, size: usize) {
        *self.record_store_size.lock().unwrap() = size;
    }

    fn record_node_issue(&self, issue: NodeIssue) {
        *self.last_issue.lock().unwrap() = Some(issue);
    }
}

#[test]
fn test_metrics_recorder() {
    let recorder = MockMetricsRecorder::default();
    
    recorder.record_close_group_size(5);
    recorder.record_connection_count(10);
    recorder.record_record_store_size(100);
    recorder.record_node_issue(NodeIssue::ConnectionFailed("test".into()));
    
    assert_eq!(*recorder.close_group_size.lock().unwrap(), 5);
    assert_eq!(*recorder.connection_count.lock().unwrap(), 10);
    assert_eq!(*recorder.record_store_size.lock().unwrap(), 100);
    assert!(matches!(
        *recorder.last_issue.lock().unwrap(),
        Some(NodeIssue::ConnectionFailed(_))
    ));
} 