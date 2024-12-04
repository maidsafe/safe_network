use crate::{
    config::NetworkConfig,
    network::{
        error::NetworkError,
        event::NetworkEvent,
    },
    types::NodeIssue,
};
use super::{RecordStorage, PeerManager};
use futures::{StreamExt, channel::mpsc};
use libp2p::{
    swarm::SwarmEvent,
    kad::{self, KademliaEvent},
    identify, PeerId, Swarm,
};
use std::{sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Driver for managing the libp2p Swarm
#[derive(Debug)]
pub struct SwarmDriver {
    /// The underlying libp2p Swarm
    swarm: Swarm<kad::Behaviour>,
    /// Channel for sending network events
    event_sender: mpsc::Sender<NetworkEvent>,
    /// Shared storage manager
    storage: Arc<RwLock<RecordStorage>>,
    /// Shared peer manager
    peer_manager: Arc<RwLock<PeerManager>>,
    /// Configuration
    config: NetworkConfig,
}

impl SwarmDriver {
    /// Creates a new SwarmDriver
    pub async fn new(
        config: NetworkConfig,
        local_peer_id: PeerId,
        storage: Arc<RwLock<RecordStorage>>,
        peer_manager: Arc<RwLock<PeerManager>>,
    ) -> Result<Self, NetworkError> {
        let (event_sender, _) = mpsc::channel(config.channel_size);
        
        // Create Kademlia behavior
        let kad_config = kad::Config::default();
        let kad_behaviour = kad::Behaviour::new(local_peer_id, kad_config);
        
        // Create and configure the Swarm
        let swarm = libp2p::SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_behaviour(|_| kad_behaviour)
            .map_err(|e| NetworkError::Other(format!("Failed to create swarm: {}", e)))?
            .build();

        Ok(Self {
            swarm,
            event_sender,
            storage,
            peer_manager,
            config,
        })
    }

    /// Starts the swarm driver
    pub async fn run(&mut self) -> Result<(), NetworkError> {
        info!("Starting swarm driver");

        loop {
            tokio::select! {
                event = self.swarm.next() => {
                    match event {
                        Some(SwarmEvent::Behaviour(kad::Event::OutboundQueryCompleted { 
                            result, ..
                        })) => {
                            self.handle_query_result(result).await?;
                        }
                        Some(SwarmEvent::ConnectionEstablished { 
                            peer_id, ..
                        }) => {
                            self.handle_peer_connected(peer_id).await?;
                        }
                        Some(SwarmEvent::ConnectionClosed { 
                            peer_id, cause, ..
                        }) => {
                            self.handle_peer_disconnected(peer_id, cause).await?;
                        }
                        Some(event) => {
                            debug!("Unhandled swarm event: {:?}", event);
                        }
                        None => break,
                    }
                }
                // Add other select branches for timeouts, maintenance, etc.
            }
        }

        Ok(())
    }

    async fn handle_query_result(
        &mut self,
        result: kad::QueryResult,
    ) -> Result<(), NetworkError> {
        match result {
            kad::QueryResult::GetRecord(Ok(ok)) => {
                if let Some(record) = ok.records.first() {
                    let event = NetworkEvent::RecordRetrieved {
                        key: record.key.clone(),
                        value: record.value.clone(),
                    };
                    self.event_sender.send(event).await.map_err(|e| {
                        NetworkError::Other(format!("Failed to send event: {}", e))
                    })?;
                }
            }
            kad::QueryResult::GetRecord(Err(err)) => {
                warn!("GetRecord query failed: {:?}", err);
                self.event_sender
                    .send(NetworkEvent::Error(NetworkError::Kademlia(format!("{:?}", err))))
                    .await
                    .map_err(|e| NetworkError::Other(format!("Failed to send error event: {}", e)))?;
            }
            // Handle other query results...
            _ => debug!("Unhandled query result: {:?}", result),
        }
        Ok(())
    }

    async fn handle_peer_connected(&mut self, peer_id: PeerId) -> Result<(), NetworkError> {
        let event = NetworkEvent::PeerConnected {
            peer_id,
            timestamp: std::time::Instant::now(),
        };
        self.event_sender.send(event).await.map_err(|e| {
            NetworkError::Other(format!("Failed to send peer connected event: {}", e))
        })?;
        
        if let Some(mut peer_manager) = self.peer_manager.try_write() {
            peer_manager.add_peer(peer_id);
        }
        
        Ok(())
    }

    async fn handle_peer_disconnected(
        &mut self,
        peer_id: PeerId,
        cause: Option<String>,
    ) -> Result<(), NetworkError> {
        let event = NetworkEvent::PeerDisconnected {
            peer_id,
            reason: cause.clone(),
        };
        self.event_sender.send(event).await.map_err(|e| {
            NetworkError::Other(format!("Failed to send peer disconnected event: {}", e))
        })?;
        
        if let Some(mut peer_manager) = self.peer_manager.try_write() {
            peer_manager.remove_peer(peer_id);
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NetworkConfig;
    use futures::StreamExt;
    use libp2p::{
        kad::{Record, store::MemoryStore},
        identity,
    };
    use std::time::Duration;

    /// Creates a test swarm driver with mock components
    async fn create_test_swarm() -> (SwarmDriver, PeerId, mpsc::Receiver<NetworkEvent>) {
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());
        let config = NetworkConfig::default();
        
        let (event_sender, event_receiver) = mpsc::channel(config.channel_size);
        let storage = Arc::new(RwLock::new(RecordStorage::new(config.clone()).unwrap()));
        let peer_manager = Arc::new(RwLock::new(PeerManager::new(config.clone()).unwrap()));

        let driver = SwarmDriver::new(
            config,
            local_peer_id,
            storage,
            peer_manager,
        ).await.unwrap();

        (driver, local_peer_id, event_receiver)
    }

    #[tokio::test]
    async fn test_swarm_creation() {
        let (driver, local_peer_id, _) = create_test_swarm().await;
        assert_eq!(driver.swarm.local_peer_id(), &local_peer_id);
    }

    #[tokio::test]
    async fn test_peer_connection_events() {
        let (mut driver, _, mut event_rx) = create_test_swarm().await;
        let peer_id = PeerId::random();

        // Simulate peer connection
        driver.handle_peer_connected(peer_id).await.unwrap();

        // Verify connection event
        if let Some(NetworkEvent::PeerConnected { peer_id: connected_peer, .. }) = event_rx.next().await {
            assert_eq!(connected_peer, peer_id);
        } else {
            panic!("Expected PeerConnected event");
        }

        // Simulate peer disconnection
        driver.handle_peer_disconnected(peer_id, Some("test disconnect".into())).await.unwrap();

        // Verify disconnection event
        if let Some(NetworkEvent::PeerDisconnected { peer_id: disconnected_peer, reason }) = event_rx.next().await {
            assert_eq!(disconnected_peer, peer_id);
            assert_eq!(reason.as_deref(), Some("test disconnect"));
        } else {
            panic!("Expected PeerDisconnected event");
        }
    }

    #[tokio::test]
    async fn test_query_handling() {
        let (mut driver, _, mut event_rx) = create_test_swarm().await;
        let key = vec![1, 2, 3];
        let value = vec![4, 5, 6];
        let record = Record {
            key: key.clone(),
            value: value.clone(),
            publisher: None,
            expires: None,
        };

        // Simulate successful GetRecord query
        let query_result = kad::QueryResult::GetRecord(Ok(kad::GetRecordOk {
            records: vec![record],
            cache_candidates: vec![],
        }));

        driver.handle_query_result(query_result).await.unwrap();

        // Verify record retrieval event
        if let Some(NetworkEvent::RecordRetrieved { key: received_key, value: received_value }) = event_rx.next().await {
            assert_eq!(received_key, key);
            assert_eq!(received_value, value);
        } else {
            panic!("Expected RecordRetrieved event");
        }

        // Simulate failed GetRecord query
        let query_result = kad::QueryResult::GetRecord(Err(kad::GetRecordError::NotFound));
        driver.handle_query_result(query_result).await.unwrap();

        // Verify error event
        if let Some(NetworkEvent::Error(NetworkError::Kademlia(_))) = event_rx.next().await {
            // Error handled correctly
        } else {
            panic!("Expected Kademlia error event");
        }
    }

    #[tokio::test]
    async fn test_swarm_shutdown() {
        let (mut driver, _, _) = create_test_swarm().await;
        
        // Run the swarm for a short time
        let driver_handle = tokio::spawn(async move {
            driver.run().await
        });

        // Wait a bit and then verify the driver can be shut down cleanly
        tokio::time::sleep(Duration::from_millis(100)).await;
        driver_handle.abort();
        
        match driver_handle.await {
            Ok(_) => panic!("Driver should have been aborted"),
            Err(e) if e.is_cancelled() => (), // Expected
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }

    #[tokio::test]
    async fn test_concurrent_events() {
        let (mut driver, _, mut event_rx) = create_test_swarm().await;
        let mut handles = vec![];

        // Spawn multiple concurrent event handlers
        for _ in 0..5 {
            let peer_id = PeerId::random();
            let driver_clone = &mut driver;
            handles.push(tokio::spawn(async move {
                driver_clone.handle_peer_connected(peer_id).await
            }));
        }

        // Wait for all events to be processed
        for handle in handles {
            handle.await.unwrap().unwrap();
        }

        // Verify all events were received
        let mut connection_count = 0;
        while let Ok(Some(event)) = tokio::time::timeout(Duration::from_millis(100), event_rx.next()).await {
            if let NetworkEvent::PeerConnected { .. } = event {
                connection_count += 1;
            }
        }

        assert_eq!(connection_count, 5);
    }
} 