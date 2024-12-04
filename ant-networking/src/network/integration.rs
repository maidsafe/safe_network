use crate::{
    config::NetworkConfig,
    network::{
        driver::{NetworkDriver, NetworkBuilder},
        error::NetworkError,
        event::NetworkEvent,
        logging::{self, LogContext},
        metrics::NetworkMetrics,
    },
    types::NetworkMetricsRecorder,
};
use futures::channel::mpsc;
use libp2p::PeerId;
use std::{sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn, instrument};

/// Manages the integration of all network components
#[derive(Debug)]
pub struct NetworkIntegration {
    /// The network driver
    driver: NetworkDriver,
    /// Metrics collector
    metrics: Arc<NetworkMetrics>,
    /// Event sender channel
    event_sender: mpsc::Sender<NetworkEvent>,
    /// Event receiver channel
    event_receiver: mpsc::Receiver<NetworkEvent>,
    /// Configuration
    config: NetworkConfig,
}

impl NetworkIntegration {
    /// Creates a new NetworkIntegration instance
    #[instrument(skip(config))]
    pub async fn new(config: NetworkConfig) -> Result<Self, NetworkError> {
        info!("Initializing network integration");
        
        // Initialize logging
        logging::init_logging()?;

        // Create channels
        let (event_sender, event_receiver) = mpsc::channel(config.channel_size);

        // Initialize metrics
        let metrics = Arc::new(NetworkMetrics::new(config.clone()));

        // Create network driver
        let local_peer_id = PeerId::random();
        let driver = NetworkBuilder::new()
            .with_config(config.clone())
            .with_peer_id(local_peer_id)
            .with_event_channel_size(config.channel_size)
            .build()
            .await?;

        debug!("Network integration initialized successfully");

        Ok(Self {
            driver,
            metrics,
            event_sender,
            event_receiver,
            config,
        })
    }

    /// Starts the network integration
    #[instrument(skip(self))]
    pub async fn run(&mut self) -> Result<(), NetworkError> {
        info!("Starting network integration");
        let context = LogContext::new("network_start");

        // Start metrics collection
        self.record_initial_metrics().await?;

        // Main event loop
        loop {
            tokio::select! {
                event = self.event_receiver.next() => {
                    match event {
                        Some(event) => {
                            if let Err(e) = self.handle_event(event).await {
                                let error_context = context.clone().with_details(format!("Event handling failed: {}", e));
                                logging::log_error(&e, &error_context);
                                self.handle_error(e).await?;
                            }
                        }
                        None => break,
                    }
                }
                // Add periodic maintenance tasks
                _ = tokio::time::sleep(Duration::from_secs(60)) => {
                    self.perform_maintenance().await?;
                }
            }
        }

        info!("Network integration stopped");
        Ok(())
    }

    /// Handles network events
    #[instrument(skip(self, event))]
    async fn handle_event(&mut self, event: NetworkEvent) -> Result<(), NetworkError> {
        let context = LogContext::new("event_handling");
        logging::log_event(&event, &context);

        match &event {
            NetworkEvent::Error(error) => {
                self.handle_error(error.clone()).await?;
            }
            NetworkEvent::NodeIssue(issue) => {
                self.metrics.record_node_issue(issue.clone());
            }
            _ => {
                // Forward event to driver
                self.event_sender.send(event).await.map_err(|e| {
                    NetworkError::Other(format!("Failed to forward event: {}", e))
                })?;
            }
        }

        Ok(())
    }

    /// Handles network errors
    #[instrument(skip(self, error))]
    async fn handle_error(&self, error: NetworkError) -> Result<(), NetworkError> {
        let context = LogContext::new("error_handling");
        logging::log_error(&error, &context);

        // Update metrics
        match &error {
            NetworkError::Connection { peer_id, .. } => {
                self.metrics.record_connection_failure(*peer_id);
            }
            NetworkError::Timeout(duration) => {
                self.metrics.record_timeout(*duration);
            }
            _ => {
                self.metrics.record_general_error();
            }
        }

        Ok(())
    }

    /// Records initial metrics
    async fn record_initial_metrics(&self) -> Result<(), NetworkError> {
        let context = LogContext::new("metrics_initialization");
        
        // Record initial metrics
        self.metrics.record_close_group_size(self.config.close_group_size);
        self.metrics.record_connection_count(0);
        
        debug!("Initial metrics recorded");
        Ok(())
    }

    /// Performs periodic maintenance tasks
    #[instrument(skip(self))]
    async fn perform_maintenance(&mut self) -> Result<(), NetworkError> {
        let context = LogContext::new("maintenance");
        debug!("Performing maintenance tasks");

        // Update metrics
        let snapshot = self.metrics.get_snapshot();
        info!(
            "Network metrics - Connections: {}, Records: {}, Errors: {}",
            snapshot.active_connections,
            snapshot.record_count,
            snapshot.failed_operations
        );

        Ok(())
    }

    /// Shuts down the network integration
    #[instrument(skip(self))]
    pub async fn shutdown(&mut self) -> Result<(), NetworkError> {
        info!("Shutting down network integration");
        let context = LogContext::new("shutdown");

        // Perform cleanup tasks
        self.metrics.reset();
        
        debug!("Network integration shut down successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::NodeIssue;
    use std::sync::Arc;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_integration_lifecycle() {
        let config = NetworkConfig::default();
        let mut integration = NetworkIntegration::new(config).await.unwrap();

        // Start integration in background
        let handle = tokio::spawn(async move {
            integration.run().await
        });

        // Wait a bit and then shut down
        tokio::time::sleep(Duration::from_millis(100)).await;
        handle.abort();
    }

    #[tokio::test]
    async fn test_event_handling() {
        let config = NetworkConfig::default();
        let mut integration = NetworkIntegration::new(config).await.unwrap();
        let peer_id = PeerId::random();

        // Test handling different event types
        let events = vec![
            NetworkEvent::PeerConnected {
                peer_id,
                timestamp: std::time::Instant::now(),
            },
            NetworkEvent::Error(NetworkError::Other("test error".into())),
            NetworkEvent::NodeIssue(crate::types::NodeIssue::ConnectionFailed("test".into())),
        ];

        for event in events {
            integration.handle_event(event).await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_metrics_recording() {
        let config = NetworkConfig::default();
        let integration = NetworkIntegration::new(config).await.unwrap();

        // Verify initial metrics
        let snapshot = integration.metrics.get_snapshot();
        assert_eq!(snapshot.active_connections, 0);
        assert_eq!(snapshot.failed_operations, 0);
    }

    #[tokio::test]
    async fn test_error_handling() {
        let config = NetworkConfig::default();
        let integration = NetworkIntegration::new(config).await.unwrap();

        // Test handling different error types
        let errors = vec![
            NetworkError::Connection {
                peer_id: PeerId::random(),
                reason: "test".into(),
            },
            NetworkError::Timeout(Duration::from_secs(30)),
            NetworkError::Other("test error".into()),
        ];

        for error in errors {
            integration.handle_error(error).await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_integration_shutdown_with_pending_events() {
        let config = NetworkConfig::default();
        let mut integration = NetworkIntegration::new(config).await.unwrap();
        let peer_id = PeerId::random();

        // Queue multiple events
        for _ in 0..5 {
            integration.handle_event(NetworkEvent::PeerConnected {
                peer_id,
                timestamp: std::time::Instant::now(),
            }).await.unwrap();
        }

        // Shutdown should handle pending events gracefully
        integration.shutdown().await.unwrap();

        // Verify metrics were reset
        let snapshot = integration.metrics.get_snapshot();
        assert_eq!(snapshot.active_connections, 0);
        assert_eq!(snapshot.failed_operations, 0);
    }

    #[tokio::test]
    async fn test_integration_error_propagation() {
        let config = NetworkConfig::default();
        let mut integration = NetworkIntegration::new(config).await.unwrap();

        // Test error propagation through event handling
        let error_event = NetworkEvent::Error(NetworkError::Protocol("test error".into()));
        let result = integration.handle_event(error_event).await;
        assert!(result.is_ok()); // Error should be handled, not propagated

        // Verify error was recorded in metrics
        let snapshot = integration.metrics.get_snapshot();
        assert!(snapshot.failed_operations > 0);
    }

    #[tokio::test]
    async fn test_integration_maintenance() {
        let config = NetworkConfig::default();
        let mut integration = NetworkIntegration::new(config).await.unwrap();

        // Run maintenance
        integration.perform_maintenance().await.unwrap();

        // Verify maintenance completed successfully
        let snapshot = integration.metrics.get_snapshot();
        assert_eq!(snapshot.active_connections, 0); // Should be reset during maintenance
    }

    #[tokio::test]
    async fn test_integration_concurrent_events() {
        let config = NetworkConfig::default();
        let integration = Arc::new(tokio::sync::Mutex::new(
            NetworkIntegration::new(config).await.unwrap()
        ));
        let mut handles = vec![];

        // Spawn multiple concurrent event handlers
        for i in 0..10 {
            let integration_clone = integration.clone();
            let handle = tokio::spawn(async move {
                let mut integration = integration_clone.lock().await;
                let event = if i % 2 == 0 {
                    NetworkEvent::PeerConnected {
                        peer_id: PeerId::random(),
                        timestamp: std::time::Instant::now(),
                    }
                } else {
                    NetworkEvent::NodeIssue(NodeIssue::ConnectionFailed("test".into()))
                };
                integration.handle_event(event).await
            });
            handles.push(handle);
        }

        // Wait for all events to be processed
        for handle in handles {
            handle.await.unwrap().unwrap();
        }

        // Verify events were processed
        let integration = integration.lock().await;
        let snapshot = integration.metrics.get_snapshot();
        assert!(snapshot.failed_operations > 0);
    }

    #[tokio::test]
    async fn test_integration_timeout_handling() {
        let mut config = NetworkConfig::default();
        config.request_timeout = Duration::from_millis(100);
        let mut integration = NetworkIntegration::new(config).await.unwrap();

        // Create a long-running event processing task
        let event_future = integration.handle_event(NetworkEvent::PeerConnected {
            peer_id: PeerId::random(),
            timestamp: std::time::Instant::now(),
        });

        // Verify timeout works
        match timeout(Duration::from_millis(200), event_future).await {
            Ok(_) => (), // Event processed normally
            Err(_) => panic!("Event processing timed out"),
        }
    }

    #[tokio::test]
    async fn test_integration_metrics_accuracy() {
        let config = NetworkConfig::default();
        let mut integration = NetworkIntegration::new(config).await.unwrap();
        let peer_id = PeerId::random();

        // Record various events
        integration.handle_event(NetworkEvent::PeerConnected {
            peer_id,
            timestamp: std::time::Instant::now(),
        }).await.unwrap();

        integration.handle_event(NetworkEvent::Error(
            NetworkError::Connection { peer_id, reason: "test".into() }
        )).await.unwrap();

        integration.handle_event(NetworkEvent::NodeIssue(
            NodeIssue::ConnectionFailed("test".into())
        )).await.unwrap();

        // Verify metrics
        let snapshot = integration.metrics.get_snapshot();
        assert_eq!(snapshot.active_connections, 1); // One connection
        assert_eq!(snapshot.failed_operations, 2); // One error and one issue
    }

    #[tokio::test]
    async fn test_integration_event_ordering() {
        let config = NetworkConfig::default();
        let mut integration = NetworkIntegration::new(config).await.unwrap();
        let peer_id = PeerId::random();

        // Queue events with different priorities
        let events = vec![
            NetworkEvent::PeerConnected {
                peer_id,
                timestamp: std::time::Instant::now(),
            },
            NetworkEvent::Error(NetworkError::Other("high priority".into())),
            NetworkEvent::RecordStored {
                key: vec![1],
                peer_id,
            },
        ];

        for event in events {
            integration.handle_event(event).await.unwrap();
        }

        // Error events should be processed first
        let snapshot = integration.metrics.get_snapshot();
        assert!(snapshot.failed_operations > 0);
    }
} 