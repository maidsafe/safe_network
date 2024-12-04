use super::*;
use libp2p::{
    identity::Keypair,
    kad::{Record, RecordKey},
    multiaddr::Protocol,
    Multiaddr,
};
use std::{net::Ipv4Addr, time::Duration};
use tokio::time::timeout;

async fn setup_test_nodes() -> (Network, Network, SwarmDriver, SwarmDriver) {
    let keypair1 = Keypair::generate_ed25519();
    let keypair2 = Keypair::generate_ed25519();
    
    let mut builder1 = NetworkBuilder::new(keypair1, true);
    let mut builder2 = NetworkBuilder::new(keypair2, true);

    // Configure nodes to listen on different ports
    builder1.listen_addr("127.0.0.1:0".parse().unwrap());
    builder2.listen_addr("127.0.0.1:0".parse().unwrap());

    let temp_dir = std::env::temp_dir();
    let (network1, _events1, driver1) = builder1.build_node(temp_dir.clone()).await.unwrap();
    let (network2, _events2, driver2) = builder2.build_node(temp_dir).await.unwrap();

    (network1, network2, driver1, driver2)
}

#[tokio::test]
async fn test_node_discovery() {
    let (network1, network2, mut driver1, mut driver2) = setup_test_nodes().await;

    // Get the listening addresses
    let addr1 = driver1.swarm.listeners().next().unwrap().clone();
    let peer1 = driver1.self_peer_id;
    
    // Connect node2 to node1
    let mut addr = addr1.clone();
    addr.push(Protocol::P2p(peer1.into()));
    network2.dial(addr).await.unwrap();

    // Wait for connection
    let timeout_duration = Duration::from_secs(5);
    timeout(timeout_duration, async {
        loop {
            if driver1.peers_in_rt > 0 && driver2.peers_in_rt > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .unwrap();

    assert!(driver1.peers_in_rt > 0);
    assert!(driver2.peers_in_rt > 0);
}

#[tokio::test]
async fn test_record_replication() {
    let (network1, network2, mut driver1, mut driver2) = setup_test_nodes().await;

    // Connect the nodes
    let addr1 = driver1.swarm.listeners().next().unwrap().clone();
    let peer1 = driver1.self_peer_id;
    let mut addr = addr1.clone();
    addr.push(Protocol::P2p(peer1.into()));
    network2.dial(addr).await.unwrap();

    // Wait for connection
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Create and store a record
    let key = RecordKey::new(&[1, 2, 3]);
    let value = vec![4, 5, 6];
    let record = Record {
        key: key.clone().into_vec(),
        value: value.clone(),
        publisher: None,
        expires: None,
    };

    let put_cfg = PutRecordCfg {
        put_quorum: Quorum::One,
        retry_strategy: None,
        use_put_record_to: None,
        verification: None,
    };

    // Store record on node1
    network1.put_record(record.clone(), put_cfg).await.unwrap();

    // Wait for replication
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Try to get record from node2
    let get_cfg = GetRecordCfg {
        get_quorum: Quorum::One,
        retry_strategy: None,
        target_record: None,
        expected_holders: Default::default(),
        is_register: false,
    };

    let result = network2.get_record(key.clone(), get_cfg).await;
    assert!(result.is_ok());
    let retrieved_record = result.unwrap();
    assert_eq!(retrieved_record.value, value);
}

#[tokio::test]
async fn test_network_metrics() {
    let (network1, _network2, driver1, _driver2) = setup_test_nodes().await;

    // Test basic metrics
    assert_eq!(driver1.peers_in_rt, 0);
    assert!(driver1.hard_disk_write_error == 0);

    // Test connection metrics
    assert!(driver1.live_connected_peers.is_empty());
    assert!(driver1.latest_established_connection_ids.is_empty());

    // More metrics tests...
}

#[tokio::test]
async fn test_error_recovery() {
    let (network1, network2, mut driver1, mut driver2) = setup_test_nodes().await;

    // Test recovery from various error conditions
    // - Connection drops
    // - Invalid records
    // - Network partitions
    // ...
}
