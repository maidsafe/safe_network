use crate::{
    config::NetworkConfig,
    network::{
        error::NetworkError,
        types::{NetworkDistance, NetworkTimeout},
    },
};
use libp2p::{
    kad::{self, Kademlia, KademliaEvent, QueryId, QueryResult},
    identify::{Identify, IdentifyEvent, IdentifyInfo},
    ping::{Ping, PingEvent},
    swarm::NetworkBehaviour,
    PeerId,
};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use tracing::{debug, info, warn};

/// Combined network behavior for the node
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "ComposedEvent")]
pub struct NetworkBehavior {
    /// Kademlia DHT behavior
    kademlia: Kademlia,
    /// Identify protocol behavior
    identify: Identify,
    /// Ping protocol behavior
    ping: Ping,
    /// Active queries and their metadata
    #[behaviour(ignore)]
    active_queries: HashMap<QueryId, QueryMetadata>,
    /// Configuration
    #[behaviour(ignore)]
    config: NetworkConfig,
}

/// Metadata for active queries
#[derive(Debug)]
struct QueryMetadata {
    /// When the query was started
    started_at: Instant,
    /// Query type
    query_type: QueryType,
    /// Target peer (if applicable)
    target_peer: Option<PeerId>,
}

/// Types of queries that can be performed
#[derive(Debug, Clone, PartialEq, Eq)]
enum QueryType {
    GetRecord,
    PutRecord,
    FindPeer,
    Bootstrap,
}

/// Events emitted by the network behavior
#[derive(Debug)]
pub enum ComposedEvent {
    /// Kademlia DHT events
    Kademlia(KademliaEvent),
    /// Identify protocol events
    Identify(IdentifyEvent),
    /// Ping protocol events
    Ping(PingEvent),
    /// Query timeout events
    QueryTimeout(QueryId),
}

impl NetworkBehavior {
    /// Creates a new NetworkBehavior instance
    pub fn new(config: NetworkConfig, local_peer_id: PeerId) -> Result<Self, NetworkError> {
        let kad_config = kad::Config::default();
        let kademlia = Kademlia::with_config(local_peer_id, kad_config);
        
        let identify = Identify::new(
            "/safe/id/1.0.0".into(),
            "safe-network".into(),
            local_peer_id.clone(),
        );

        let ping = Ping::default();

        Ok(Self {
            kademlia,
            identify,
            ping,
            active_queries: HashMap::new(),
            config,
        })
    }

    /// Starts a get record query
    pub fn get_record(&mut self, key: Vec<u8>) -> QueryId {
        let query_id = self.kademlia.get_record(key);
        self.active_queries.insert(
            query_id,
            QueryMetadata {
                started_at: Instant::now(),
                query_type: QueryType::GetRecord,
                target_peer: None,
            },
        );
        query_id
    }

    /// Starts a put record query
    pub fn put_record(&mut self, record: kad::Record) -> QueryId {
        let query_id = self.kademlia.put_record(record, kad::Quorum::All);
        self.active_queries.insert(
            query_id,
            QueryMetadata {
                started_at: Instant::now(),
                query_type: QueryType::PutRecord,
                target_peer: None,
            },
        );
        query_id
    }

    /// Starts a find peer query
    pub fn find_peer(&mut self, peer_id: PeerId) -> QueryId {
        let query_id = self.kademlia.get_closest_peers(peer_id);
        self.active_queries.insert(
            query_id,
            QueryMetadata {
                started_at: Instant::now(),
                query_type: QueryType::FindPeer,
                target_peer: Some(peer_id),
            },
        );
        query_id
    }

    /// Bootstraps the node into the network
    pub fn bootstrap(&mut self) -> QueryId {
        let query_id = self.kademlia.bootstrap();
        self.active_queries.insert(
            query_id,
            QueryMetadata {
                started_at: Instant::now(),
                query_type: QueryType::Bootstrap,
                target_peer: None,
            },
        );
        query_id
    }

    /// Checks for timed out queries
    pub fn check_timeouts(&mut self) -> Vec<QueryId> {
        let now = Instant::now();
        let timeout = self.config.kad_query_timeout;
        let mut timed_out = Vec::new();

        self.active_queries.retain(|query_id, metadata| {
            let is_active = now.duration_since(metadata.started_at) < timeout;
            if !is_active {
                timed_out.push(*query_id);
                warn!("Query {:?} timed out after {:?}", query_id, timeout);
            }
            is_active
        });

        timed_out
    }

    /// Handles a completed query
    pub fn handle_query_result(&mut self, query_id: QueryId, result: QueryResult) {
        if let Some(metadata) = self.active_queries.remove(&query_id) {
            debug!(
                "Query {:?} completed after {:?}: {:?}",
                query_id,
                metadata.started_at.elapsed(),
                result
            );
        }
    }

    /// Updates the node's identity information
    pub fn update_identity(&mut self, info: IdentifyInfo) {
        self.identify.push_identify(info);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_behavior_creation() {
        let config = NetworkConfig::default();
        let local_peer_id = PeerId::random();
        
        let behavior = NetworkBehavior::new(config, local_peer_id);
        assert!(behavior.is_ok());
    }

    #[test]
    fn test_query_management() {
        let config = NetworkConfig::default();
        let local_peer_id = PeerId::random();
        let mut behavior = NetworkBehavior::new(config, local_peer_id).unwrap();

        // Start different types of queries
        let get_query = behavior.get_record(vec![1, 2, 3]);
        let put_query = behavior.put_record(kad::Record {
            key: vec![4, 5, 6],
            value: vec![7, 8, 9],
            publisher: None,
            expires: None,
        });
        let find_query = behavior.find_peer(PeerId::random());
        let bootstrap_query = behavior.bootstrap();

        // Verify queries are tracked
        assert_eq!(behavior.active_queries.len(), 4);
        assert!(behavior.active_queries.contains_key(&get_query));
        assert!(behavior.active_queries.contains_key(&put_query));
        assert!(behavior.active_queries.contains_key(&find_query));
        assert!(behavior.active_queries.contains_key(&bootstrap_query));
    }

    #[test]
    fn test_query_timeouts() {
        let mut config = NetworkConfig::default();
        config.kad_query_timeout = Duration::from_millis(100);
        
        let local_peer_id = PeerId::random();
        let mut behavior = NetworkBehavior::new(config, local_peer_id).unwrap();

        // Start a query
        let query_id = behavior.get_record(vec![1, 2, 3]);
        assert_eq!(behavior.active_queries.len(), 1);

        // Wait for timeout
        std::thread::sleep(Duration::from_millis(200));

        // Check timeouts
        let timed_out = behavior.check_timeouts();
        assert_eq!(timed_out.len(), 1);
        assert_eq!(timed_out[0], query_id);
        assert!(behavior.active_queries.is_empty());
    }

    #[test]
    fn test_query_completion() {
        let config = NetworkConfig::default();
        let local_peer_id = PeerId::random();
        let mut behavior = NetworkBehavior::new(config, local_peer_id).unwrap();

        // Start a query
        let query_id = behavior.get_record(vec![1, 2, 3]);
        assert_eq!(behavior.active_queries.len(), 1);

        // Complete the query
        let result = QueryResult::GetRecord(Ok(kad::GetRecordOk {
            records: vec![],
            cache_candidates: vec![],
        }));
        behavior.handle_query_result(query_id, result);

        // Verify query was removed
        assert!(behavior.active_queries.is_empty());
    }
} 