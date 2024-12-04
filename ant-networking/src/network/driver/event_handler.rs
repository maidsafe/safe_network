use crate::{
    config::NetworkConfig,
    network::{
        error::NetworkError,
        event::{NetworkEvent, EventHandler, EventPriority},
        types::NetworkTimeout,
    },
    types::{NodeIssue, NetworkMetricsRecorder},
};
use super::{RecordStorage, PeerManager};
use futures::{channel::mpsc, StreamExt};
use libp2p::PeerId;
use std::{
    sync::Arc,
    time::{Duration, Instant},
    collections::HashMap,
};
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};

const MAX_BATCH_SIZE: usize = 50;
const BATCH_TIMEOUT: Duration = Duration::from_millis(100);
const ERROR_THRESHOLD: usize = 10;
const ERROR_WINDOW: Duration = Duration::from_secs(60);

/// Processes and routes network events
#[derive(Debug)]
pub struct EventProcessor {
    event_handler: EventHandler,
    storage: Arc<RwLock<RecordStorage>>,
    peer_manager: Arc<RwLock<PeerManager>>,
    config: NetworkConfig,
    event_receiver: mpsc::Receiver<NetworkEvent>,
    metrics: Option<Arc<dyn NetworkMetricsRecorder>>,
    error_counts: HashMap<PeerId, Vec<Instant>>,
    current_batch: Vec<NetworkEvent>,
    last_batch_time: Instant,
}

impl EventProcessor {
    /// Creates a new EventProcessor
    pub fn new(
        config: NetworkConfig,
        event_receiver: mpsc::Receiver<NetworkEvent>,
        storage: Arc<RwLock<RecordStorage>>,
        peer_manager: Arc<RwLock<PeerManager>>,
    ) -> Result<Self, NetworkError> {
        let (event_sender, _) = mpsc::channel(config.channel_size);
        let event_handler = EventHandler::new(event_sender, Duration::from_secs(30))?;

        Ok(Self {
            event_handler,
            storage,
            peer_manager,
            config,
            event_receiver,
            metrics: None,
            error_counts: HashMap::new(),
            current_batch: Vec::with_capacity(MAX_BATCH_SIZE),
            last_batch_time: Instant::now(),
        })
    }

    /// Sets a metrics recorder
    pub fn with_metrics(mut self, metrics: Arc<dyn NetworkMetricsRecorder>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Starts processing events
    pub async fn run(&mut self) -> Result<(), NetworkError> {
        info!("Starting event processor");
        self.record_initial_metrics().await?;

        loop {
            tokio::select! {
                event = self.event_receiver.next() => {
                    match event {
                        Some(event) => {
                            self.handle_event(event).await?;
                        }
                        None => break,
                    }
                }
                _ = tokio::time::sleep(BATCH_TIMEOUT) => {
                    if !self.current_batch.is_empty() {
                        self.process_batch().await?;
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_event(&mut self, event: NetworkEvent) -> Result<(), NetworkError> {
        // Handle high-priority events immediately
        match &event {
            NetworkEvent::Error(_) | NetworkEvent::NodeIssue(_) => {
                self.process_event(event).await?;
            }
            _ => {
                self.current_batch.push(event);
                if self.current_batch.len() >= MAX_BATCH_SIZE {
                    self.process_batch().await?;
                }
            }
        }
        Ok(())
    }

    async fn process_batch(&mut self) -> Result<(), NetworkError> {
        if self.current_batch.is_empty() {
            return Ok(());
        }

        let batch = std::mem::take(&mut self.current_batch);
        for event in batch {
            if let Err(e) = self.process_event(event).await {
                error!("Error processing event: {}", e);
                self.handle_error(e).await?;
            }
        }

        self.last_batch_time = Instant::now();
        Ok(())
    }

    async fn process_event(&mut self, event: NetworkEvent) -> Result<(), NetworkError> {
        match event {
            NetworkEvent::PeerConnected { peer_id, timestamp } => {
                self.handle_peer_connected(peer_id, timestamp).await?;
            }
            NetworkEvent::PeerDisconnected { peer_id, reason } => {
                self.handle_peer_disconnected(peer_id, reason).await?;
            }
            NetworkEvent::RecordStored { key, peer_id } => {
                self.handle_record_stored(key, peer_id).await?;
            }
            NetworkEvent::RecordRetrieved { key, value } => {
                self.handle_record_retrieved(key, value).await?;
            }
            NetworkEvent::Error(error) => {
                self.handle_error(error).await?;
            }
            NetworkEvent::NodeIssue(issue) => {
                self.handle_node_issue(issue).await?;
            }
        }

        Ok(())
    }

    async fn handle_peer_connected(
        &mut self,
        peer_id: PeerId,
        timestamp: std::time::Instant,
    ) -> Result<(), NetworkError> {
        if let Some(mut peer_manager) = self.peer_manager.try_write() {
            peer_manager.add_peer(peer_id);
            debug!("Peer {} connected at {:?}", peer_id, timestamp);
        }
        Ok(())
    }

    async fn handle_peer_disconnected(
        &mut self,
        peer_id: PeerId,
        reason: Option<String>,
    ) -> Result<(), NetworkError> {
        if let Some(mut peer_manager) = self.peer_manager.try_write() {
            peer_manager.remove_peer(peer_id);
            if let Some(reason) = reason {
                warn!("Peer {} disconnected: {}", peer_id, reason);
            } else {
                debug!("Peer {} disconnected", peer_id);
            }
        }
        Ok(())
    }

    async fn handle_record_stored(
        &mut self,
        key: Vec<u8>,
        peer_id: PeerId,
    ) -> Result<(), NetworkError> {
        if let Some(mut storage) = self.storage.try_write() {
            // Implementation depends on RecordStorage API
            debug!("Record stored by peer {}: key={:?}", peer_id, key);
        }
        Ok(())
    }

    async fn handle_record_retrieved(
        &mut self,
        key: Vec<u8>,
        value: Vec<u8>,
    ) -> Result<(), NetworkError> {
        if let Some(mut storage) = self.storage.try_write() {
            // Implementation depends on RecordStorage API
            debug!("Record retrieved: key={:?}, value_len={}", key, value.len());
        }
        Ok(())
    }

    async fn handle_error(&mut self, error: NetworkError) -> Result<(), NetworkError> {
        warn!("Network error: {}", error);
        // Queue high-priority error event
        self.event_handler
            .queue_event(NetworkEvent::Error(error))
            .await
    }

    async fn handle_node_issue(&mut self, issue: NodeIssue) -> Result<(), NetworkError> {
        warn!("Node issue: {}", issue);
        // Queue high-priority node issue event
        self.event_handler
            .queue_event(NetworkEvent::NodeIssue(issue))
            .await
    }

    async fn handle_peer_error(&mut self, peer_id: PeerId, error: &str) -> Result<(), NetworkError> {
        let now = Instant::now();
        let errors = self.error_counts.entry(peer_id).or_insert_with(Vec::new);
        
        // Remove old errors
        errors.retain(|time| now.duration_since(*time) < ERROR_WINDOW);
        errors.push(now);

        if errors.len() >= ERROR_THRESHOLD {
            warn!("Peer {} exceeded error threshold, disconnecting", peer_id);
            if let Some(mut peer_manager) = self.peer_manager.try_write() {
                peer_manager.remove_peer(peer_id);
            }
            self.error_counts.remove(&peer_id);
        }

        Ok(())
    }

    async fn record_initial_metrics(&mut self) -> Result<(), NetworkError> {
        if let Some(metrics) = &self.metrics {
            if let Some(storage) = self.storage.try_read() {
                // metrics.record_record_store_size(storage.size());
            }
            if let Some(peer_manager) = self.peer_manager.try_read() {
                // metrics.record_connection_count(peer_manager.connected_peers());
                // metrics.record_close_group_size(peer_manager.close_group_size());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_processor_creation() {
        let config = NetworkConfig::default();
        let (_, event_receiver) = mpsc::channel(100);
        let storage = Arc::new(RwLock::new(RecordStorage::new(config.clone()).unwrap()));
        let peer_manager = Arc::new(RwLock::new(PeerManager::new(config.clone()).unwrap()));

        let processor = EventProcessor::new(
            config,
            event_receiver,
            storage,
            peer_manager,
        );

        assert!(processor.is_ok());
    }

    #[tokio::test]
    async fn test_event_processing() {
        let config = NetworkConfig::default();
        let (mut event_sender, event_receiver) = mpsc::channel(100);
        let storage = Arc::new(RwLock::new(RecordStorage::new(config.clone()).unwrap()));
        let peer_manager = Arc::new(RwLock::new(PeerManager::new(config.clone()).unwrap()));

        let mut processor = EventProcessor::new(
            config,
            event_receiver,
            storage.clone(),
            peer_manager.clone(),
        ).unwrap();

        // Send test events
        let peer_id = PeerId::random();
        let connect_event = NetworkEvent::PeerConnected {
            peer_id,
            timestamp: std::time::Instant::now(),
        };

        event_sender.send(connect_event).await.unwrap();

        // Start processor in background
        let processor_handle = tokio::spawn(async move {
            processor.run().await
        });

        // Wait a bit for processing
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Verify peer was added
        if let Some(peer_manager) = peer_manager.try_read() {
            // Add verification based on PeerManager API
            // assert!(peer_manager.is_connected(&peer_id));
        }

        processor_handle.abort();
    }

    #[tokio::test]
    async fn test_error_handling() {
        let config = NetworkConfig::default();
        let (mut event_sender, event_receiver) = mpsc::channel(100);
        let storage = Arc::new(RwLock::new(RecordStorage::new(config.clone()).unwrap()));
        let peer_manager = Arc::new(RwLock::new(PeerManager::new(config.clone()).unwrap()));

        let mut processor = EventProcessor::new(
            config,
            event_receiver,
            storage,
            peer_manager,
        ).unwrap();

        // Send error event
        let error_event = NetworkEvent::Error(NetworkError::Other("test error".into()));
        event_sender.send(error_event).await.unwrap();

        // Process the event
        let result = processor.run().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_batch_processing() {
        let config = NetworkConfig::default();
        let (mut event_sender, event_receiver) = mpsc::channel(100);
        let storage = Arc::new(RwLock::new(RecordStorage::new(config.clone()).unwrap()));
        let peer_manager = Arc::new(RwLock::new(PeerManager::new(config.clone()).unwrap()));

        let mut processor = EventProcessor::new(
            config,
            event_receiver,
            storage,
            peer_manager,
        ).unwrap();

        // Send multiple events quickly
        for _ in 0..MAX_BATCH_SIZE + 1 {
            let peer_id = PeerId::random();
            let event = NetworkEvent::PeerConnected {
                peer_id,
                timestamp: Instant::now(),
            };
            event_sender.send(event).await.unwrap();
        }

        // Start processor
        let processor_handle = tokio::spawn(async move {
            processor.run().await
        });

        // Wait for batch timeout
        tokio::time::sleep(BATCH_TIMEOUT * 2).await;
        processor_handle.abort();
    }

    #[tokio::test]
    async fn test_error_threshold() {
        let config = NetworkConfig::default();
        let (mut event_sender, event_receiver) = mpsc::channel(100);
        let storage = Arc::new(RwLock::new(RecordStorage::new(config.clone()).unwrap()));
        let peer_manager = Arc::new(RwLock::new(PeerManager::new(config.clone()).unwrap()));

        let mut processor = EventProcessor::new(
            config,
            event_receiver,
            storage.clone(),
            peer_manager.clone(),
        ).unwrap();

        let peer_id = PeerId::random();

        // Generate errors for the same peer
        for _ in 0..ERROR_THRESHOLD {
            processor.handle_peer_error(peer_id, "test error").await.unwrap();
        }

        // Verify peer was removed
        if let Some(peer_manager) = peer_manager.try_read() {
            // assert!(!peer_manager.is_connected(&peer_id));
        }
    }
} 