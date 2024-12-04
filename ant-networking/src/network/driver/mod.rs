//! Network driver module handling core networking functionality

mod builder;
mod swarm;
mod event_handler;
mod storage;
mod peer_management;

#[cfg(test)]
mod tests;

pub use builder::NetworkBuilder;
pub use swarm::SwarmDriver;
pub use event_handler::EventProcessor;
pub use storage::RecordStorage;
pub use peer_management::PeerManager;

use crate::{
    config::NetworkConfig,
    network::{error::NetworkError, event::NetworkEvent},
};
use futures::channel::mpsc;
use libp2p::PeerId;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Core driver managing network operations
#[derive(Debug)]
pub struct NetworkDriver {
    /// Configuration for the network
    config: NetworkConfig,
    /// Local peer ID
    local_peer_id: PeerId,
    /// Swarm driver handling libp2p operations
    swarm_driver: SwarmDriver,
    /// Event processor handling network events
    event_processor: EventProcessor,
    /// Record storage manager
    storage: Arc<RwLock<RecordStorage>>,
    /// Peer management
    peer_manager: Arc<RwLock<PeerManager>>,
    /// Event sender channel
    event_sender: mpsc::Sender<NetworkEvent>,
} 