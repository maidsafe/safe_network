use crate::{
    network::{error::NetworkError, types::NetworkTimeout},
    types::NodeIssue,
};
use futures::channel::mpsc;
use libp2p::PeerId;
use std::{collections::{VecDeque, BTreeMap}, time::{Duration, Instant}};
use tokio::sync::oneshot;
use tracing::{debug, warn, info};

/// Maximum number of events that can be queued
const MAX_QUEUED_EVENTS: usize = 1000;
/// Maximum batch size for processing events
const MAX_BATCH_SIZE: usize = 50;
/// Time window for batching events (milliseconds)
const BATCH_WINDOW_MS: u64 = 100;

/// Priority levels for network events
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventPriority {
    High,
    Normal,
    Low,
}

/// Network events that can occur
#[derive(Debug)]
pub enum NetworkEvent {
    /// A new peer connected
    PeerConnected {
        peer_id: PeerId,
        timestamp: Instant,
    },
    /// A peer disconnected
    PeerDisconnected {
        peer_id: PeerId,
        reason: Option<String>,
    },
    /// A record was stored
    RecordStored {
        key: Vec<u8>,
        peer_id: PeerId,
    },
    /// A record was retrieved
    RecordRetrieved {
        key: Vec<u8>,
        value: Vec<u8>,
    },
    /// An error occurred
    Error(NetworkError),
    /// Node issue detected
    NodeIssue(NodeIssue),
}

impl NetworkEvent {
    /// Returns the priority level for this event
    fn priority(&self) -> EventPriority {
        match self {
            NetworkEvent::Error(_) | NetworkEvent::NodeIssue(_) => EventPriority::High,
            NetworkEvent::PeerConnected { .. } | NetworkEvent::PeerDisconnected { .. } => EventPriority::Normal,
            _ => EventPriority::Low,
        }
    }
}

/// A batch of network events
#[derive(Debug)]
struct EventBatch {
    events: Vec<NetworkEvent>,
    created_at: Instant,
}

/// Handler for network events with prioritization and batching
#[derive(Debug)]
pub struct EventHandler {
    /// Priority queues for events
    priority_queues: BTreeMap<EventPriority, VecDeque<NetworkEvent>>,
    /// Current batch being built
    current_batch: EventBatch,
    /// Channel for sending events to the network
    event_sender: mpsc::Sender<Vec<NetworkEvent>>,
    /// Maximum queue size per priority level
    max_queue_size: usize,
    /// Event processing timeout
    timeout: NetworkTimeout,
}

impl EventHandler {
    /// Creates a new EventHandler
    pub fn new(
        event_sender: mpsc::Sender<Vec<NetworkEvent>>,
        timeout: Duration,
    ) -> Result<Self, NetworkError> {
        Ok(Self {
            priority_queues: BTreeMap::new(),
            current_batch: EventBatch {
                events: Vec::with_capacity(MAX_BATCH_SIZE),
                created_at: Instant::now(),
            },
            event_sender,
            max_queue_size: MAX_QUEUED_EVENTS,
            timeout: NetworkTimeout::new(timeout).map_err(|e| NetworkError::Config(e.into()))?,
        })
    }

    /// Queues an event for processing with priority handling
    pub async fn queue_event(&mut self, event: NetworkEvent) -> Result<(), NetworkError> {
        let priority = event.priority();
        let queue = self.priority_queues
            .entry(priority)
            .or_insert_with(|| VecDeque::with_capacity(self.max_queue_size));

        if queue.len() >= self.max_queue_size {
            warn!("Queue full for priority {:?}, dropping oldest event", priority);
            queue.pop_front();
        }

        queue.push_back(event);
        self.try_process_batch().await
    }

    /// Attempts to process a batch of events
    async fn try_process_batch(&mut self) -> Result<(), NetworkError> {
        let now = Instant::now();
        let batch_window = Duration::from_millis(BATCH_WINDOW_MS);

        // Check if current batch should be sent
        if self.current_batch.events.len() >= MAX_BATCH_SIZE 
            || (now - self.current_batch.created_at) >= batch_window {
            self.send_current_batch().await?;
        }

        // Fill new batch from priority queues
        while self.current_batch.events.len() < MAX_BATCH_SIZE {
            let mut found_event = false;
            
            // Process events in priority order
            for priority in [EventPriority::High, EventPriority::Normal, EventPriority::Low] {
                if let Some(queue) = self.priority_queues.get_mut(&priority) {
                    if let Some(event) = queue.pop_front() {
                        self.current_batch.events.push(event);
                        found_event = true;
                        break;
                    }
                }
            }

            if !found_event {
                break;
            }
        }

        Ok(())
    }

    /// Sends the current batch of events
    async fn send_current_batch(&mut self) -> Result<(), NetworkError> {
        if self.current_batch.events.is_empty() {
            return Ok(());
        }

        let events = std::mem::replace(&mut self.current_batch.events, Vec::with_capacity(MAX_BATCH_SIZE));
        let (tx, rx) = oneshot::channel();
        
        // Try to send the batch with timeout
        let send_future = self.event_sender.clone().send(events);
        let timeout = tokio::time::sleep(self.timeout.duration());
        
        tokio::select! {
            send_result = send_future => {
                if let Err(e) = send_result {
                    warn!("Failed to send event batch: {}", e);
                    return Err(NetworkError::Other(format!("Event batch send error: {}", e)));
                }
                let _ = tx.send(());
            }
            _ = timeout => {
                warn!("Event batch processing timed out");
                return Err(NetworkError::Timeout(self.timeout.duration()));
            }
        }

        // Wait for confirmation
        if rx.await.is_err() {
            warn!("Event batch confirmation channel closed");
            return Err(NetworkError::Other("Event batch confirmation failed".into()));
        }

        self.current_batch.created_at = Instant::now();
        debug!("Event batch processed successfully");
        Ok(())
    }

    /// Returns the total number of queued events across all priorities
    pub fn queued_events(&self) -> usize {
        self.priority_queues.values().map(|q| q.len()).sum()
    }

    /// Clears all event queues
    pub fn clear_queues(&mut self) {
        self.priority_queues.clear();
        self.current_batch.events.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    #[tokio::test]
    async fn test_event_handler() {
        let (tx, mut rx) = mpsc::channel(10);
        let mut handler = EventHandler::new(tx, Duration::from_secs(5)).unwrap();

        // Queue some events
        let peer_id = PeerId::random();
        let event = NetworkEvent::PeerConnected {
            peer_id,
            timestamp: std::time::Instant::now(),
        };

        handler.queue_event(event.clone()).await.unwrap();
        assert_eq!(handler.queued_events(), 0); // Event should be processed immediately

        // Verify event was received
        if let Some(received) = rx.next().await {
            match received {
                NetworkEvent::PeerConnected { peer_id: p, .. } => assert_eq!(p, peer_id),
                _ => panic!("Unexpected event type"),
            }
        } else {
            panic!("No event received");
        }
    }

    #[tokio::test]
    async fn test_event_queue_backpressure() {
        let (tx, mut rx) = mpsc::channel(1);
        let mut handler = EventHandler::new(tx, Duration::from_secs(1)).unwrap();

        // Fill the queue
        for _ in 0..MAX_QUEUED_EVENTS + 1 {
            let event = NetworkEvent::PeerConnected {
                peer_id: PeerId::random(),
                timestamp: std::time::Instant::now(),
            };
            handler.queue_event(event).await.unwrap();
        }

        // Verify oldest event was dropped
        assert!(handler.queued_events() < MAX_QUEUED_EVENTS);

        // Drain events
        while rx.next().await.is_some() {}
    }
} 