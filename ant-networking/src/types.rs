use libp2p::PeerId;
use std::collections::HashSet;

/// Represents a network address with additional metadata
#[derive(Debug, Clone)]
pub struct NetworkAddress {
    /// The peer ID associated with this network address
    pub peer_id: PeerId,
    /// Set of peers that hold this address
    pub holders: HashSet<PeerId>,
    /// Distance metric from the local node
    pub distance: u32,
}

impl NetworkAddress {
    /// Creates a new NetworkAddress instance
    ///
    /// # Arguments
    /// * `peer_id` - The peer ID for this address
    /// * `distance` - Distance metric from local node
    pub fn new(peer_id: PeerId, distance: u32) -> Self {
        Self {
            peer_id,
            holders: HashSet::new(),
            distance,
        }
    }

    /// Adds a holder to this network address
    ///
    /// # Arguments
    /// * `holder` - The peer ID of the holder to add
    pub fn add_holder(&mut self, holder: PeerId) {
        self.holders.insert(holder);
    }

    /// Returns the number of holders for this address
    pub fn holder_count(&self) -> usize {
        self.holders.len()
    }
}

/// Represents issues that can occur with a node
#[derive(Debug, Clone)]
pub enum NodeIssue {
    /// Connection to the node failed
    ConnectionFailed(String),
    /// Failed to store or retrieve records
    RecordStoreFailed(String),
    /// General network-related error
    NetworkError(String),
}

impl std::fmt::Display for NodeIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeIssue::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            NodeIssue::RecordStoreFailed(msg) => write!(f, "Record store failed: {}", msg),
            NodeIssue::NetworkError(msg) => write!(f, "Network error: {}", msg),
        }
    }
}

/// A trait for handling network metrics
pub trait NetworkMetricsRecorder: Send + Sync {
    /// Record the current size of the close group
    fn record_close_group_size(&self, size: usize);
    /// Record the current number of connections
    fn record_connection_count(&self, count: usize);
    /// Record the current size of the record store
    fn record_record_store_size(&self, size: usize);
    /// Record an issue that occurred with a node
    fn record_node_issue(&self, issue: NodeIssue);
}
