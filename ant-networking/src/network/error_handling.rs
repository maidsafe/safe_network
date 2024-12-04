use crate::{
    network::{error::NetworkError, event::NetworkEvent},
    types::NodeIssue,
};
use libp2p::PeerId;
use std::{
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Maximum number of retries for operations
const MAX_RETRIES: usize = 3;
/// Time window for error rate calculation
const ERROR_WINDOW: Duration = Duration::from_secs(300); // 5 minutes
/// Maximum error rate before triggering circuit breaker
const MAX_ERROR_RATE: f64 = 0.5; // 50%

/// Tracks error rates and implements circuit breaker pattern
#[derive(Debug)]
pub struct CircuitBreaker {
    /// Error counts within the time window
    errors: AtomicUsize,
    /// Operation counts within the time window
    operations: AtomicUsize,
    /// Last reset time
    last_reset: RwLock<Instant>,
    /// Whether the circuit is open (blocking operations)
    is_open: AtomicUsize,
}

impl CircuitBreaker {
    /// Creates a new CircuitBreaker
    pub fn new() -> Self {
        Self {
            errors: AtomicUsize::new(0),
            operations: AtomicUsize::new(0),
            last_reset: RwLock::new(Instant::now()),
            is_open: AtomicUsize::new(0),
        }
    }

    /// Records an operation attempt
    pub fn record_operation(&self) {
        self.operations.fetch_add(1, Ordering::Relaxed);
    }

    /// Records an error
    pub fn record_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
        self.check_threshold();
    }

    /// Checks if the circuit is open
    pub fn is_open(&self) -> bool {
        self.is_open.load(Ordering::Relaxed) != 0
    }

    /// Resets the circuit breaker
    pub async fn reset(&self) {
        *self.last_reset.write().await = Instant::now();
        self.errors.store(0, Ordering::Relaxed);
        self.operations.store(0, Ordering::Relaxed);
        self.is_open.store(0, Ordering::Relaxed);
    }

    fn check_threshold(&self) {
        let errors = self.errors.load(Ordering::Relaxed);
        let operations = self.operations.load(Ordering::Relaxed);

        if operations > 0 {
            let error_rate = errors as f64 / operations as f64;
            if error_rate > MAX_ERROR_RATE {
                self.is_open.store(1, Ordering::Relaxed);
                warn!("Circuit breaker opened: error rate {}", error_rate);
            }
        }
    }
}

/// Manages error handling and recovery strategies
#[derive(Debug)]
pub struct ErrorHandler {
    /// Circuit breakers per peer
    circuit_breakers: HashMap<PeerId, CircuitBreaker>,
    /// Retry counts per operation
    retry_counts: HashMap<String, usize>,
    /// Last error times per peer
    last_errors: HashMap<PeerId, Instant>,
}

impl ErrorHandler {
    /// Creates a new ErrorHandler
    pub fn new() -> Self {
        Self {
            circuit_breakers: HashMap::new(),
            retry_counts: HashMap::new(),
            last_errors: HashMap::new(),
        }
    }

    /// Handles a network error
    pub async fn handle_error(
        &mut self,
        error: &NetworkError,
        peer_id: Option<PeerId>,
    ) -> Result<(), NetworkError> {
        // Record error timing
        if let Some(peer_id) = peer_id {
            self.last_errors.insert(peer_id, Instant::now());
            
            // Update circuit breaker
            let breaker = self.circuit_breakers
                .entry(peer_id)
                .or_insert_with(CircuitBreaker::new);
            breaker.record_error();
        }

        // Log error with context
        match error {
            NetworkError::Connection { peer_id, reason } => {
                warn!("Connection error for peer {}: {}", peer_id, reason);
                self.handle_connection_error(*peer_id, reason).await?;
            }
            NetworkError::Timeout(duration) => {
                warn!("Operation timed out after {:?}", duration);
                self.handle_timeout_error(duration).await?;
            }
            NetworkError::Protocol(msg) => {
                error!("Protocol error: {}", msg);
                self.handle_protocol_error(msg).await?;
            }
            _ => {
                error!("Network error: {}", error);
            }
        }

        Ok(())
    }

    /// Checks if an operation should be retried
    pub fn should_retry(&self, operation: &str) -> bool {
        self.retry_counts
            .get(operation)
            .map_or(true, |&count| count < MAX_RETRIES)
    }

    /// Records a retry attempt
    pub fn record_retry(&mut self, operation: String) {
        *self.retry_counts.entry(operation).or_insert(0) += 1;
    }

    /// Checks if a peer is experiencing issues
    pub fn is_peer_healthy(&self, peer_id: &PeerId) -> bool {
        if let Some(breaker) = self.circuit_breakers.get(peer_id) {
            if breaker.is_open() {
                return false;
            }
        }

        if let Some(last_error) = self.last_errors.get(peer_id) {
            if last_error.elapsed() < ERROR_WINDOW {
                return false;
            }
        }

        true
    }

    async fn handle_connection_error(&mut self, peer_id: PeerId, reason: &str) -> Result<(), NetworkError> {
        warn!("Handling connection error for peer {}: {}", peer_id, reason);
        
        // Create node issue event
        let issue = NodeIssue::ConnectionFailed(reason.to_string());
        
        // Implement recovery strategy
        if self.should_retry("connection") {
            self.record_retry("connection".to_string());
            debug!("Retrying connection for peer {}", peer_id);
            // Retry logic here
        } else {
            warn!("Max retries reached for peer {}", peer_id);
        }

        Ok(())
    }

    async fn handle_timeout_error(&self, duration: &Duration) -> Result<(), NetworkError> {
        warn!("Handling timeout error after {:?}", duration);
        // Implement timeout recovery strategy
        Ok(())
    }

    async fn handle_protocol_error(&self, msg: &str) -> Result<(), NetworkError> {
        error!("Handling protocol error: {}", msg);
        // Implement protocol error recovery strategy
        Ok(())
    }

    /// Resets error state for a peer
    pub async fn reset_peer(&mut self, peer_id: &PeerId) {
        if let Some(breaker) = self.circuit_breakers.get(peer_id) {
            breaker.reset().await;
        }
        self.last_errors.remove(peer_id);
        debug!("Reset error state for peer {}", peer_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_circuit_breaker() {
        let breaker = CircuitBreaker::new();
        
        // Record some operations and errors
        for _ in 0..10 {
            breaker.record_operation();
        }
        for _ in 0..6 {
            breaker.record_error();
        }

        // Circuit should be open due to high error rate
        assert!(breaker.is_open());

        // Reset should clear the state
        breaker.reset().await;
        assert!(!breaker.is_open());
    }

    #[tokio::test]
    async fn test_error_handler() {
        let mut handler = ErrorHandler::new();
        let peer_id = PeerId::random();

        // Test connection error handling
        let error = NetworkError::Connection {
            peer_id,
            reason: "test error".into(),
        };
        handler.handle_error(&error, Some(peer_id)).await.unwrap();

        // Peer should be marked as unhealthy
        assert!(!handler.is_peer_healthy(&peer_id));

        // Reset should restore peer health
        handler.reset_peer(&peer_id).await;
        assert!(handler.is_peer_healthy(&peer_id));
    }

    #[tokio::test]
    async fn test_retry_mechanism() {
        let mut handler = ErrorHandler::new();
        let operation = "test_operation".to_string();

        // Initial attempt should be allowed
        assert!(handler.should_retry(&operation));

        // Record retries
        for _ in 0..MAX_RETRIES {
            handler.record_retry(operation.clone());
        }

        // Should not retry after max attempts
        assert!(!handler.should_retry(&operation));
    }

    #[tokio::test]
    async fn test_error_rate_threshold() {
        let breaker = CircuitBreaker::new();

        // Record operations with high error rate
        for _ in 0..10 {
            breaker.record_operation();
            breaker.record_error();
        }

        assert!(breaker.is_open());

        // Reset and record operations with low error rate
        breaker.reset().await;
        for _ in 0..10 {
            breaker.record_operation();
        }
        breaker.record_error();

        assert!(!breaker.is_open());
    }
} 