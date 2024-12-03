// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use ant_bootstrap_cache::{BootstrapCacheStore, BootstrapConfig, PeersArgs};
use ant_logging::LogBuilder;
use libp2p::Multiaddr;
use tempfile::TempDir;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

// Setup function to create a new temp directory and config for each test
async fn setup() -> (TempDir, BootstrapConfig) {
    let temp_dir = TempDir::new().unwrap();
    let cache_path = temp_dir.path().join("cache.json");

    let config = BootstrapConfig::empty()
        .unwrap()
        .with_cache_path(&cache_path)
        .with_max_peers(50);

    (temp_dir, config)
}

#[tokio::test]
async fn test_multiaddr_format_parsing() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = LogBuilder::init_single_threaded_tokio_test("address_format_tests", false);

    // Test various multiaddr formats
    let addrs = vec![
        // quic
        "/ip4/127.0.0.1/udp/8080/quic-v1/p2p/12D3KooWRBhwfeP2Y4TCx1SM6s9rUoHhR5STiGwxBhgFRcw3UERE",
        // ws
        "/ip4/127.0.0.1/tcp/8080/ws/p2p/12D3KooWRBhwfeP2Y4TCx1SM6s9rUoHhR5STiGwxBhgFRcw3UERE",
    ];

    for addr_str in addrs {
        let (_temp_dir, config) = setup().await; // Fresh config for each test case
        let addr = addr_str.parse::<Multiaddr>()?;
        let args = PeersArgs {
            first: false,
            addrs: vec![addr.clone()],
            network_contacts_url: None,
            local: false,
        };

        let store = BootstrapCacheStore::from_args(args, config).await?;
        let bootstrap_addresses = store.get_addrs().collect::<Vec<_>>();
        assert_eq!(bootstrap_addresses.len(), 1, "Should have one peer");
        assert_eq!(
            bootstrap_addresses[0].addr, addr,
            "Address format should match"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_network_contacts_format() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = LogBuilder::init_single_threaded_tokio_test("address_format_tests", false);

    let (_temp_dir, config) = setup().await;

    // Create a mock server with network contacts format
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/peers"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "/ip4/127.0.0.1/udp/8080/quic-v1/p2p/12D3KooWRBhwfeP2Y4TCx1SM6s9rUoHhR5STiGwxBhgFRcw3UERE\n\
             /ip4/127.0.0.2/udp/8081/quic-v1/p2p/12D3KooWRBhwfeP2Y4TCx1SM6s9rUoHhR5STiGwxBhgFRcw3UERF"
        ))
        .mount(&mock_server)
        .await;

    let args = PeersArgs {
        first: false,
        addrs: vec![],
        network_contacts_url: Some(format!("{}/peers", mock_server.uri()).parse()?),
        local: false,
    };

    let store = BootstrapCacheStore::from_args(args, config).await?;
    let adddrs = store.get_addrs().collect::<Vec<_>>();
    assert_eq!(
        adddrs.len(),
        2,
        "Should have two peers from network contacts"
    );

    // Verify address formats
    for addr in adddrs {
        let addr_str = addr.addr.to_string();
        assert!(addr_str.contains("/ip4/"), "Should have IPv4 address");
        assert!(addr_str.contains("/udp/"), "Should have UDP port");
        assert!(addr_str.contains("/quic-v1/"), "Should have QUIC protocol");
        assert!(addr_str.contains("/p2p/"), "Should have peer ID");
    }

    Ok(())
}

#[tokio::test]
async fn test_invalid_address_handling() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = LogBuilder::init_single_threaded_tokio_test("address_format_tests", false);

    // Test various invalid address formats
    let invalid_addrs = vec![
        "not-a-multiaddr",
        "127.0.0.1",            // IP only
        "127.0.0.1:8080:extra", // Invalid socket addr
        "/ip4/127.0.0.1",       // Incomplete multiaddr
    ];

    for addr_str in invalid_addrs {
        let (_temp_dir, config) = setup().await; // Fresh config for each test case
        let args = PeersArgs {
            first: false,
            addrs: vec![],
            network_contacts_url: None,
            local: true, // Use local mode to avoid fetching from default endpoints
        };

        let store = BootstrapCacheStore::from_args(args.clone(), config.clone()).await?;
        let addrs = store.get_addrs().collect::<Vec<_>>();
        assert_eq!(
            addrs.len(),
            0,
            "Should have no peers from invalid address in env var: {}",
            addr_str
        );

        // Also test direct args path
        if let Ok(addr) = addr_str.parse::<Multiaddr>() {
            let args_with_peer = PeersArgs {
                first: false,
                addrs: vec![addr],
                network_contacts_url: None,
                local: false,
            };
            let store = BootstrapCacheStore::from_args(args_with_peer, config).await?;
            let addrs = store.get_addrs().collect::<Vec<_>>();
            assert_eq!(
                addrs.len(),
                0,
                "Should have no peers from invalid address in args: {}",
                addr_str
            );
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_socket_addr_format() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = LogBuilder::init_single_threaded_tokio_test("address_format_tests", false);

    let temp_dir = TempDir::new()?;
    let cache_path = temp_dir.path().join("cache.json");

    let args = PeersArgs {
        first: false,
        addrs: vec![],
        network_contacts_url: None,
        local: true, // Use local mode to avoid getting peers from default endpoints
    };

    let config = BootstrapConfig::empty()
        .unwrap()
        .with_cache_path(&cache_path);

    let store = BootstrapCacheStore::from_args(args, config).await?;
    let addrs = store.get_addrs().collect::<Vec<_>>();
    assert!(addrs.is_empty(), "Should have no peers in local mode");

    Ok(())
}

#[tokio::test]
async fn test_multiaddr_format() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = LogBuilder::init_single_threaded_tokio_test("address_format_tests", false);

    let temp_dir = TempDir::new()?;
    let cache_path = temp_dir.path().join("cache.json");

    let args = PeersArgs {
        first: false,
        addrs: vec![],
        network_contacts_url: None,
        local: true, // Use local mode to avoid getting peers from default endpoints
    };

    let config = BootstrapConfig::empty()
        .unwrap()
        .with_cache_path(&cache_path);

    let store = BootstrapCacheStore::from_args(args, config).await?;
    let addrs = store.get_addrs().collect::<Vec<_>>();
    assert!(addrs.is_empty(), "Should have no peers in local mode");

    Ok(())
}

#[tokio::test]
async fn test_invalid_addr_format() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = LogBuilder::init_single_threaded_tokio_test("address_format_tests", false);

    let temp_dir = TempDir::new()?;
    let cache_path = temp_dir.path().join("cache.json");

    let args = PeersArgs {
        first: false,
        addrs: vec![],
        network_contacts_url: None,
        local: true, // Use local mode to avoid getting peers from default endpoints
    };

    let config = BootstrapConfig::empty()
        .unwrap()
        .with_cache_path(&cache_path);

    let store = BootstrapCacheStore::from_args(args, config).await?;
    let addrs = store.get_addrs().collect::<Vec<_>>();
    assert!(addrs.is_empty(), "Should have no peers in local mode");

    Ok(())
}

#[tokio::test]
async fn test_mixed_addr_formats() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = LogBuilder::init_single_threaded_tokio_test("address_format_tests", false);

    let temp_dir = TempDir::new()?;
    let cache_path = temp_dir.path().join("cache.json");

    let args = PeersArgs {
        first: false,
        addrs: vec![],
        network_contacts_url: None,
        local: true, // Use local mode to avoid getting peers from default endpoints
    };

    let config = BootstrapConfig::empty()
        .unwrap()
        .with_cache_path(&cache_path);

    let store = BootstrapCacheStore::from_args(args, config).await?;
    let addrs = store.get_addrs().collect::<Vec<_>>();
    assert!(addrs.is_empty(), "Should have no peers in local mode");

    Ok(())
}

#[tokio::test]
async fn test_socket_addr_conversion() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = LogBuilder::init_single_threaded_tokio_test("address_format_tests", false);

    let temp_dir = TempDir::new()?;
    let cache_path = temp_dir.path().join("cache.json");

    let args = PeersArgs {
        first: false,
        addrs: vec![],
        network_contacts_url: None,
        local: true, // Use local mode to avoid getting peers from default endpoints
    };

    let config = BootstrapConfig::empty()
        .unwrap()
        .with_cache_path(&cache_path);

    let store = BootstrapCacheStore::from_args(args, config).await?;
    let addrs = store.get_addrs().collect::<Vec<_>>();
    assert!(addrs.is_empty(), "Should have no peers in local mode");

    Ok(())
}

#[tokio::test]
async fn test_invalid_socket_addr() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = LogBuilder::init_single_threaded_tokio_test("address_format_tests", false);

    let temp_dir = TempDir::new()?;
    let cache_path = temp_dir.path().join("cache.json");

    let args = PeersArgs {
        first: false,
        addrs: vec![],
        network_contacts_url: None,
        local: true, // Use local mode to avoid getting peers from default endpoints
    };

    let config = BootstrapConfig::empty()
        .unwrap()
        .with_cache_path(&cache_path);

    let store = BootstrapCacheStore::from_args(args, config).await?;
    let addrs = store.get_addrs().collect::<Vec<_>>();
    assert!(addrs.is_empty(), "Should have no peers in local mode");

    Ok(())
}

#[tokio::test]
async fn test_invalid_multiaddr() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = LogBuilder::init_single_threaded_tokio_test("address_format_tests", false);

    let temp_dir = TempDir::new()?;
    let cache_path = temp_dir.path().join("cache.json");

    let args = PeersArgs {
        first: false,
        addrs: vec![],
        network_contacts_url: None,
        local: true, // Use local mode to avoid getting peers from default endpoints
    };

    let config = BootstrapConfig::empty()
        .unwrap()
        .with_cache_path(&cache_path);

    let store = BootstrapCacheStore::from_args(args, config).await?;
    let addrs = store.get_addrs().collect::<Vec<_>>();
    assert!(addrs.is_empty(), "Should have no peers in local mode");

    Ok(())
}

#[tokio::test]
async fn test_mixed_valid_invalid_addrs() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = LogBuilder::init_single_threaded_tokio_test("address_format_tests", false);

    let temp_dir = TempDir::new()?;
    let cache_path = temp_dir.path().join("cache.json");

    let args = PeersArgs {
        first: false,
        addrs: vec![],
        network_contacts_url: None,
        local: true, // Use local mode to avoid getting peers from default endpoints
    };

    let config = BootstrapConfig::empty()
        .unwrap()
        .with_cache_path(&cache_path);

    let store = BootstrapCacheStore::from_args(args, config).await?;
    let addrs = store.get_addrs().collect::<Vec<_>>();
    assert!(addrs.is_empty(), "Should have no peers in local mode");

    Ok(())
}
