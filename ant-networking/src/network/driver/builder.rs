use super::{NetworkDriver, SwarmDriver, EventProcessor, RecordStorage, PeerManager};
use crate::{
    config::NetworkConfig,
    network::{error::NetworkError, event::NetworkEvent},
};
use futures::channel::mpsc;
use libp2p::PeerId;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Builder for NetworkDriver
pub struct NetworkBuilder {
    config: Option<NetworkConfig>,
    local_peer_id: Option<PeerId>,
    event_channel_size: usize,
}

impl NetworkBuilder {
    /// Creates a new NetworkBuilder
    pub fn new() -> Self {
        Self {
            config: None,
            local_peer_id: None,
            event_channel_size: 100,
        }
    }

    /// Sets the network configuration
    pub fn with_config(mut self, config: NetworkConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Sets the local peer ID
    pub fn with_peer_id(mut self, peer_id: PeerId) -> Self {
        self.local_peer_id = Some(peer_id);
        self
    }

    /// Sets the event channel size
    pub fn with_event_channel_size(mut self, size: usize) -> Self {
        self.event_channel_size = size;
        self
    }

    /// Builds the NetworkDriver
    pub async fn build(self) -> Result<NetworkDriver, NetworkError> {
        let config = self.config.ok_or_else(|| {
            NetworkError::Config(crate::config::ConfigError::InvalidCloseGroupSize(0))
        })?;

        let local_peer_id = self.local_peer_id.ok_or_else(|| {
            NetworkError::Other("Local peer ID not provided".into())
        })?;

        let (event_sender, event_receiver) = mpsc::channel(self.event_channel_size);

        let storage = Arc::new(RwLock::new(RecordStorage::new(config.clone())?));
        let peer_manager = Arc::new(RwLock::new(PeerManager::new(config.clone())?));
        
        let swarm_driver = SwarmDriver::new(
            config.clone(),
            local_peer_id,
            storage.clone(),
            peer_manager.clone(),
        ).await?;

        let event_processor = EventProcessor::new(
            config.clone(),
            event_receiver,
            storage.clone(),
            peer_manager.clone(),
        )?;

        info!("Network driver built successfully with peer ID: {}", local_peer_id);

        Ok(NetworkDriver {
            config,
            local_peer_id,
            swarm_driver,
            event_processor,
            storage,
            peer_manager,
            event_sender,
        })
    }
}

impl Default for NetworkBuilder {
    fn default() -> Self {
        Self::new()
    }
} 