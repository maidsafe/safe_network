use crate::{
    config::NetworkConfig,
    network::error::NetworkError,
    types::{NetworkMetricsRecorder, NodeIssue},
};
use std::{
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
    time::{Duration, Instant},
};
use tracing::{debug, info, warn, error, instrument};

/// Metrics collector for network operations
#[derive(Debug)]
pub struct NetworkMetrics {
    /// Total number of bytes sent
    bytes_sent: AtomicU64,
    /// Total number of bytes received
    bytes_received: AtomicU64,
    /// Number of active connections
    active_connections: AtomicUsize,
    /// Number of records in storage
    record_count: AtomicUsize,
    /// Number of failed operations
    failed_operations: AtomicUsize,
    /// Start time of metrics collection
    start_time: Instant,
    /// Configuration
    config: NetworkConfig,
}

impl NetworkMetrics {
    /// Creates a new NetworkMetrics instance
    pub fn new(config: NetworkConfig) -> Self {
        Self {
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            active_connections: AtomicUsize::new(0),
            record_count: AtomicUsize::new(0),
            failed_operations: AtomicUsize::new(0),
            start_time: Instant::now(),
            config,
        }
    }

    /// Records bytes sent
    #[instrument(level = "debug", skip(self))]
    pub fn record_bytes_sent(&self, bytes: u64) {
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
        debug!("Recorded {} bytes sent", bytes);
    }

    /// Records bytes received
    #[instrument(level = "debug", skip(self))]
    pub fn record_bytes_received(&self, bytes: u64) {
        self.bytes_received.fetch_add(bytes, Ordering::Relaxed);
        debug!("Recorded {} bytes received", bytes);
    }

    /// Gets current metrics as a snapshot
    pub fn get_snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            active_connections: self.active_connections.load(Ordering::Relaxed),
            record_count: self.record_count.load(Ordering::Relaxed),
            failed_operations: self.failed_operations.load(Ordering::Relaxed),
            uptime: self.start_time.elapsed(),
        }
    }

    /// Resets all metrics
    pub fn reset(&self) {
        self.bytes_sent.store(0, Ordering::Relaxed);
        self.bytes_received.store(0, Ordering::Relaxed);
        self.active_connections.store(0, Ordering::Relaxed);
        self.record_count.store(0, Ordering::Relaxed);
        self.failed_operations.store(0, Ordering::Relaxed);
        info!("Metrics reset");
    }
}

impl NetworkMetricsRecorder for NetworkMetrics {
    fn record_close_group_size(&self, size: usize) {
        debug!("Close group size: {}", size);
    }

    fn record_connection_count(&self, count: usize) {
        self.active_connections.store(count, Ordering::Relaxed);
        debug!("Connection count: {}", count);
    }

    fn record_record_store_size(&self, size: usize) {
        self.record_count.store(size, Ordering::Relaxed);
        debug!("Record store size: {}", size);
    }

    fn record_node_issue(&self, issue: NodeIssue) {
        self.failed_operations.fetch_add(1, Ordering::Relaxed);
        warn!("Node issue recorded: {}", issue);
    }
}

/// Snapshot of current metrics
#[derive(Debug, Clone, Copy)]
pub struct MetricsSnapshot {
    /// Total bytes sent
    pub bytes_sent: u64,
    /// Total bytes received
    pub bytes_received: u64,
    /// Current active connections
    pub active_connections: usize,
    /// Current record count
    pub record_count: usize,
    /// Total failed operations
    pub failed_operations: usize,
    /// Time since metrics collection started
    pub uptime: Duration,
}

impl MetricsSnapshot {
    /// Calculates bytes sent per second
    pub fn bytes_sent_per_second(&self) -> f64 {
        self.bytes_sent as f64 / self.uptime.as_secs_f64()
    }

    /// Calculates bytes received per second
    pub fn bytes_received_per_second(&self) -> f64 {
        self.bytes_received as f64 / self.uptime.as_secs_f64()
    }

    /// Calculates failure rate
    pub fn failure_rate(&self) -> f64 {
        if self.record_count == 0 {
            0.0
        } else {
            self.failed_operations as f64 / self.record_count as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_metrics_recording() {
        let config = NetworkConfig::default();
        let metrics = NetworkMetrics::new(config);

        // Record some metrics
        metrics.record_bytes_sent(1000);
        metrics.record_bytes_received(500);
        metrics.record_connection_count(5);
        metrics.record_record_store_size(100);
        metrics.record_node_issue(NodeIssue::ConnectionFailed("test".into()));

        // Get snapshot
        let snapshot = metrics.get_snapshot();

        assert_eq!(snapshot.bytes_sent, 1000);
        assert_eq!(snapshot.bytes_received, 500);
        assert_eq!(snapshot.active_connections, 5);
        assert_eq!(snapshot.record_count, 100);
        assert_eq!(snapshot.failed_operations, 1);
    }

    #[test]
    fn test_metrics_reset() {
        let config = NetworkConfig::default();
        let metrics = NetworkMetrics::new(config);

        // Record some metrics
        metrics.record_bytes_sent(1000);
        metrics.record_connection_count(5);

        // Reset metrics
        metrics.reset();

        // Get snapshot
        let snapshot = metrics.get_snapshot();

        assert_eq!(snapshot.bytes_sent, 0);
        assert_eq!(snapshot.active_connections, 0);
    }

    #[test]
    fn test_metrics_calculations() {
        let snapshot = MetricsSnapshot {
            bytes_sent: 1000,
            bytes_received: 500,
            active_connections: 5,
            record_count: 100,
            failed_operations: 10,
            uptime: Duration::from_secs(10),
        };

        assert_eq!(snapshot.bytes_sent_per_second(), 100.0);
        assert_eq!(snapshot.bytes_received_per_second(), 50.0);
        assert_eq!(snapshot.failure_rate(), 0.1);
    }

    #[test]
    fn test_concurrent_metrics() {
        use std::sync::Arc;
        use std::thread;

        let config = NetworkConfig::default();
        let metrics = Arc::new(NetworkMetrics::new(config));
        let mut handles = vec![];

        // Spawn multiple threads recording metrics
        for _ in 0..5 {
            let metrics_clone = Arc::clone(&metrics);
            handles.push(thread::spawn(move || {
                metrics_clone.record_bytes_sent(100);
                metrics_clone.record_bytes_received(50);
            }));
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        let snapshot = metrics.get_snapshot();
        assert_eq!(snapshot.bytes_sent, 500);
        assert_eq!(snapshot.bytes_received, 250);
    }
} 