use crate::{
    network::{error::NetworkError, event::NetworkEvent},
    types::NodeIssue,
};
use libp2p::PeerId;
use std::{fmt, time::Instant};
use tracing::{debug, error, info, warn, Level, span, Instrument};
use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};

/// Log context for network operations
#[derive(Debug)]
pub struct LogContext {
    /// Operation start time
    start_time: Instant,
    /// Operation name
    operation: String,
    /// Associated peer ID (if any)
    peer_id: Option<PeerId>,
    /// Additional context
    details: Option<String>,
}

impl LogContext {
    /// Creates a new LogContext
    pub fn new(operation: impl Into<String>) -> Self {
        Self {
            start_time: Instant::now(),
            operation: operation.into(),
            peer_id: None,
            details: None,
        }
    }

    /// Adds a peer ID to the context
    pub fn with_peer(mut self, peer_id: PeerId) -> Self {
        self.peer_id = Some(peer_id);
        self
    }

    /// Adds details to the context
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// Returns the elapsed time since operation start
    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }
}

impl fmt::Display for LogContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Operation: {}", self.operation)?;
        if let Some(peer_id) = &self.peer_id {
            write!(f, ", Peer: {}", peer_id)?;
        }
        if let Some(details) = &self.details {
            write!(f, ", Details: {}", details)?;
        }
        write!(f, ", Elapsed: {:?}", self.elapsed())
    }
}

/// Initializes the logging system
pub fn init_logging() -> Result<(), NetworkError> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_span_events(FmtSpan::CLOSE)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .try_init()
        .map_err(|e| NetworkError::Other(format!("Failed to initialize logging: {}", e)))
}

/// Logs a network event with context
pub fn log_event(event: &NetworkEvent, context: &LogContext) {
    let span = span!(Level::INFO, "network_event", 
        operation = %context.operation,
        peer_id = ?context.peer_id,
        elapsed = ?context.elapsed()
    );

    match event {
        NetworkEvent::PeerConnected { peer_id, timestamp } => {
            info!(parent: &span, "Peer connected: {}, timestamp: {:?}", peer_id, timestamp);
        }
        NetworkEvent::PeerDisconnected { peer_id, reason } => {
            if let Some(reason) = reason {
                warn!(parent: &span, "Peer disconnected: {}, reason: {}", peer_id, reason);
            } else {
                info!(parent: &span, "Peer disconnected: {}", peer_id);
            }
        }
        NetworkEvent::RecordStored { key, peer_id } => {
            debug!(parent: &span, "Record stored by peer {}: key={:?}", peer_id, key);
        }
        NetworkEvent::RecordRetrieved { key, value } => {
            debug!(parent: &span, "Record retrieved: key={:?}, value_len={}", key, value.len());
        }
        NetworkEvent::Error(error) => {
            error!(parent: &span, "Network error: {}", error);
        }
        NetworkEvent::NodeIssue(issue) => {
            warn!(parent: &span, "Node issue: {}", issue);
        }
    }
}

/// Logs a network error with context
pub fn log_error(error: &NetworkError, context: &LogContext) {
    let span = span!(Level::ERROR, "network_error",
        operation = %context.operation,
        peer_id = ?context.peer_id,
        elapsed = ?context.elapsed()
    );

    error!(parent: &span, "Error: {}", error);
    if let Some(details) = &context.details {
        error!(parent: &span, "Details: {}", details);
    }
}

/// Logs a node issue with context
pub fn log_issue(issue: &NodeIssue, context: &LogContext) {
    let span = span!(Level::WARN, "node_issue",
        operation = %context.operation,
        peer_id = ?context.peer_id,
        elapsed = ?context.elapsed()
    );

    warn!(parent: &span, "Issue: {}", issue);
    if let Some(details) = &context.details {
        warn!(parent: &span, "Details: {}", details);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tracing_test::traced_test;

    #[test]
    #[traced_test]
    fn test_log_context() {
        let context = LogContext::new("test_operation")
            .with_peer(PeerId::random())
            .with_details("test details");

        std::thread::sleep(Duration::from_millis(10));
        assert!(context.elapsed() >= Duration::from_millis(10));
        
        let display = format!("{}", context);
        assert!(display.contains("test_operation"));
        assert!(display.contains("test details"));
    }

    #[test]
    #[traced_test]
    fn test_log_event() {
        let context = LogContext::new("test_event");
        let peer_id = PeerId::random();

        // Test connection event
        let event = NetworkEvent::PeerConnected {
            peer_id,
            timestamp: Instant::now(),
        };
        log_event(&event, &context);
        assert!(logs_contain("Peer connected"));

        // Test error event
        let event = NetworkEvent::Error(NetworkError::Other("test error".into()));
        log_event(&event, &context);
        assert!(logs_contain("Network error"));
    }

    #[test]
    #[traced_test]
    fn test_log_error() {
        let context = LogContext::new("test_error")
            .with_details("error details");
        let error = NetworkError::Other("test error".into());

        log_error(&error, &context);
        assert!(logs_contain("test error"));
        assert!(logs_contain("error details"));
    }

    #[test]
    #[traced_test]
    fn test_log_issue() {
        let context = LogContext::new("test_issue")
            .with_details("issue details");
        let issue = NodeIssue::ConnectionFailed("test failure".into());

        log_issue(&issue, &context);
        assert!(logs_contain("test failure"));
        assert!(logs_contain("issue details"));
    }
} 