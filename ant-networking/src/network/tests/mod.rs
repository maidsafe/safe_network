use super::*;
use libp2p::identity::Keypair;
use std::{net::SocketAddr, time::Duration};
use tokio::test;

mod builder_tests {
    use super::*;

    #[test]
    async fn test_network_builder_configuration() {
        let keypair = Keypair::generate_ed25519();
        let mut builder = NetworkBuilder::new(keypair, false);
        
        // Test configuration methods
        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        builder.listen_addr(addr);
        builder.request_timeout(Duration::from_secs(30));
        builder.concurrency_limit(10);
        
        // Build and verify configuration
        let temp_dir = std::env::temp_dir();
        let (network, events, driver) = builder.build_node(temp_dir).await.unwrap();
        
        // Verify configuration was applied
        assert_eq!(driver.local, false);
        // Add more assertions
    }

    #[test]
    async fn test_network_builder_errors() {
        let keypair = Keypair::generate_ed25519();
        let builder = NetworkBuilder::new(keypair, false);
        
        // Test invalid configurations
        let result = builder.build_node(PathBuf::from("/nonexistent")).await;
        assert!(result.is_err());
    }
}

mod swarm_tests {
    use super::*;

    #[test]
    async fn test_swarm_event_handling() {
        // Setup test swarm
        let keypair = Keypair::generate_ed25519();
        let builder = NetworkBuilder::new(keypair, true);
        let temp_dir = std::env::temp_dir();
        let (network, mut events, mut driver) = builder.build_node(temp_dir).await.unwrap();

        // Test event handling
        tokio::spawn(async move {
            driver.run().await;
        });

        // Verify events are processed correctly
        if let Some(event) = events.recv().await {
            // Add assertions about event handling
        }
    }
}

mod integration_tests {
    use super::*;

    #[test]
    async fn test_network_communication() {
        // Setup two nodes
        let keypair1 = Keypair::generate_ed25519();
        let keypair2 = Keypair::generate_ed25519();
        
        let builder1 = NetworkBuilder::new(keypair1, true);
        let builder2 = NetworkBuilder::new(keypair2, true);
        
        let temp_dir = std::env::temp_dir();
        let (network1, events1, driver1) = builder1.build_node(temp_dir.clone()).await.unwrap();
        let (network2, events2, driver2) = builder2.build_node(temp_dir).await.unwrap();

        // Test communication between nodes
        tokio::spawn(async move {
            driver1.run().await;
        });
        
        tokio::spawn(async move {
            driver2.run().await;
        });

        // Add communication tests
    }
}
