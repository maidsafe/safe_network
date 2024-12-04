use super::*;
use crate::{
    config::NetworkConfig,
    network::{error::NetworkError, event::NetworkEvent},
    types::NodeIssue,
};
use futures::channel::mpsc;
use libp2p::PeerId;
use std::time::Duration;

#[tokio::test]
async fn test_network_builder() {
    let peer_id = PeerId::random();
    let config = NetworkConfig::default();
    
    let builder = NetworkBuilder::new()
        .with_peer_id(peer_id)
        .with_config(config)
        .with_event_channel_size(200);
        
    assert!(builder.build().await.is_ok());
}

#[tokio::test]
async fn test_network_builder_validation() {
    // Test missing peer ID
    let config = NetworkConfig::default();
    let result = NetworkBuilder::new()
        .with_config(config.clone())
        .build()
        .await;
    assert!(matches!(
        result,
        Err(NetworkError::Other(msg)) if msg.contains("peer ID")
    ));

    // Test missing config
    let peer_id = PeerId::random();
    let result = NetworkBuilder::new()
        .with_peer_id(peer_id)
        .build()
        .await;
    assert!(matches!(result, Err(NetworkError::Config(_))));
}

#[tokio::test]
async fn test_network_driver_creation() {
    let peer_id = PeerId::random();
    let config = NetworkConfig::default();
    
    let driver = NetworkBuilder::new()
        .with_peer_id(peer_id)
        .with_config(config)
        .build()
        .await
        .unwrap();
        
    assert_eq!(driver.local_peer_id, peer_id);
}

// Mock implementations for testing
struct MockSwarmDriver;
struct MockEventProcessor;
struct MockRecordStorage;
struct MockPeerManager;

impl MockSwarmDriver {
    async fn new(
        _config: NetworkConfig,
        _peer_id: PeerId,
        _storage: Arc<RwLock<RecordStorage>>,
        _peer_manager: Arc<RwLock<PeerManager>>,
    ) -> Result<Self, NetworkError> {
        Ok(Self)
    }
}

impl MockEventProcessor {
    fn new(
        _config: NetworkConfig,
        _event_receiver: mpsc::Receiver<NetworkEvent>,
        _storage: Arc<RwLock<RecordStorage>>,
        _peer_manager: Arc<RwLock<PeerManager>>,
    ) -> Result<Self, NetworkError> {
        Ok(Self)
    }
}

impl MockRecordStorage {
    fn new(_config: NetworkConfig) -> Result<Self, NetworkError> {
        Ok(Self)
    }
}

impl MockPeerManager {
    fn new(_config: NetworkConfig) -> Result<Self, NetworkError> {
        Ok(Self)
    }
}

#[tokio::test]
async fn test_network_driver_with_mocks() {
    let peer_id = PeerId::random();
    let config = NetworkConfig::default();
    let (event_sender, _event_receiver) = mpsc::channel(100);

    let storage = Arc::new(RwLock::new(MockRecordStorage::new(config.clone()).unwrap()));
    let peer_manager = Arc::new(RwLock::new(MockPeerManager::new(config.clone()).unwrap()));
    
    let driver = NetworkDriver {
        config: config.clone(),
        local_peer_id: peer_id,
        swarm_driver: MockSwarmDriver::new(
            config.clone(),
            peer_id,
            storage.clone(),
            peer_manager.clone(),
        ).await.unwrap(),
        event_processor: MockEventProcessor::new(
            config,
            _event_receiver,
            storage.clone(),
            peer_manager.clone(),
        ).unwrap(),
        storage,
        peer_manager,
        event_sender,
    };

    assert_eq!(driver.local_peer_id, peer_id);
} 