// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use ant_bootstrap::ANT_PEERS_ENV;
use ant_bootstrap::{BootstrapCacheConfig, BootstrapCacheStore, PeersArgs};
use ant_logging::LogBuilder;
use libp2p::Multiaddr;
use std::env;
use std::fs;
use tempfile::TempDir;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

async fn setup() -> (TempDir, BootstrapCacheConfig) {
    let temp_dir = TempDir::new().unwrap();
    let cache_path = temp_dir.path().join("cache.json");
    let config = BootstrapCacheConfig::empty().with_cache_path(&cache_path);

    (temp_dir, config)
}

#[tokio::test]
async fn test_first_flag() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = LogBuilder::init_single_threaded_tokio_test("cli_integration_tests", false);
    let (_temp_dir, config) = setup().await;

    let args = PeersArgs {
        first: true,
        addrs: vec![],
        network_contacts_url: None,
        local: false,
        disable_mainnet_contacts: false,
        ignore_cache: false,
    };

    let mut store = BootstrapCacheStore::empty(config.clone())?;
    store.initialize_from_peers_arg(&args).await?;
    let addrs = store.get_addrs().collect::<Vec<_>>();
    assert!(addrs.is_empty(), "First node should have no addrs");

    Ok(())
}

#[tokio::test]
async fn test_peer_argument() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = LogBuilder::init_single_threaded_tokio_test("cli_integration_tests", false);
    let (_temp_dir, config) = setup().await;

    let peer_addr: Multiaddr =
        "/ip4/127.0.0.1/udp/8080/quic-v1/p2p/12D3KooWRBhwfeP2Y4TCx1SM6s9rUoHhR5STiGwxBhgFRcw3UERE"
            .parse()?;

    let args = PeersArgs {
        first: false,
        addrs: vec![peer_addr.clone()],
        network_contacts_url: None,
        local: false,
        disable_mainnet_contacts: false,
        ignore_cache: false,
    };

    let mut store = BootstrapCacheStore::empty(config.clone())?;
    store.initialize_from_peers_arg(&args).await?;
    let addrs = store.get_addrs().collect::<Vec<_>>();
    assert_eq!(addrs.len(), 1, "Should have one addr");
    assert_eq!(addrs[0].addr, peer_addr, "Should have the correct address");

    Ok(())
}

#[tokio::test]
async fn test_ant_peers_env() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = LogBuilder::init_single_threaded_tokio_test("cli_integration_tests", false);

    let temp_dir = TempDir::new()?;
    let cache_path = temp_dir.path().join("cache.json");

    // Set ANT_PEERS_ENV environment variable
    let addr =
        "/ip4/127.0.0.1/udp/8080/quic-v1/p2p/12D3KooWRBhwfeP2Y4TCx1SM6s9rUoHhR5STiGwxBhgFRcw3UERE";
    env::set_var(ANT_PEERS_ENV, addr);

    let args = PeersArgs {
        first: false,
        addrs: vec![],
        network_contacts_url: None,
        local: false,
        disable_mainnet_contacts: false,
        ignore_cache: false,
    };

    let config = BootstrapCacheConfig::empty().with_cache_path(&cache_path);

    let mut store = BootstrapCacheStore::empty(config.clone())?;
    store.initialize_from_peers_arg(&args).await?;
    let addrs = store.get_addrs().collect::<Vec<_>>();

    // We should have multiple peers (env var + cache/endpoints)
    assert!(!addrs.is_empty(), "Should have peers");

    // Verify that our env var peer is included in the set
    let has_env_peer = addrs.iter().any(|p| p.addr.to_string() == addr);
    assert!(
        has_env_peer,
        "Should include the peer from ANT_PEERS_ENV var"
    );

    // Clean up
    env::remove_var(ANT_PEERS_ENV);

    Ok(())
}

#[tokio::test]
async fn test_network_contacts_fallback() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = LogBuilder::init_single_threaded_tokio_test("cli_integration_tests", false);

    let (_temp_dir, config) = setup().await;

    // Start mock server
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
        disable_mainnet_contacts: false,
        ignore_cache: false,
    };

    let mut store = BootstrapCacheStore::empty(config.clone())?;
    store.initialize_from_peers_arg(&args).await?;
    let addrs = store.get_addrs().collect::<Vec<_>>();
    assert_eq!(
        addrs.len(),
        2,
        "Should have two peers from network contacts"
    );

    Ok(())
}

#[tokio::test]
async fn test_local_mode() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = LogBuilder::init_single_threaded_tokio_test("cli_integration_tests", false);

    let temp_dir = TempDir::new()?;
    let cache_path = temp_dir.path().join("cache.json");

    // Create a config with some peers in the cache
    let config = BootstrapCacheConfig::empty().with_cache_path(&cache_path);

    // Create args with local mode enabled
    let args = PeersArgs {
        first: false,
        addrs: vec![],
        network_contacts_url: None,
        local: true,
        disable_mainnet_contacts: false,
        ignore_cache: false,
    };

    let mut store = BootstrapCacheStore::empty(config.clone())?;
    store.initialize_from_peers_arg(&args).await?;
    let addrs = store.get_addrs().collect::<Vec<_>>();
    assert!(addrs.is_empty(), "Local mode should have no peers");

    // Verify cache was not touched
    assert!(
        !cache_path.exists(),
        "Cache file should not exist in local mode"
    );

    Ok(())
}

#[tokio::test]
async fn test_test_network_peers() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = LogBuilder::init_single_threaded_tokio_test("cli_integration_tests", false);

    let temp_dir = TempDir::new()?;
    let cache_path = temp_dir.path().join("cache.json");

    let peer_addr: Multiaddr =
        "/ip4/127.0.0.1/udp/8080/quic-v1/p2p/12D3KooWRBhwfeP2Y4TCx1SM6s9rUoHhR5STiGwxBhgFRcw3UERE"
            .parse()?;

    let config = BootstrapCacheConfig::empty().with_cache_path(&cache_path);

    let args = PeersArgs {
        first: false,
        addrs: vec![peer_addr.clone()],
        network_contacts_url: None,
        local: false,
        disable_mainnet_contacts: false,
        ignore_cache: false,
    };

    let mut store = BootstrapCacheStore::empty(config.clone())?;
    store.initialize_from_peers_arg(&args).await?;
    let addrs = store.get_addrs().collect::<Vec<_>>();
    assert_eq!(addrs.len(), 1, "Should have exactly one test network peer");
    assert_eq!(
        addrs[0].addr, peer_addr,
        "Should have the correct test network peer"
    );

    // Verify cache was updated
    assert!(
        cache_path.exists(),
        "Cache file should not exist for test network"
    );

    Ok(())
}

#[tokio::test]
async fn test_peers_update_cache() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = LogBuilder::init_single_threaded_tokio_test("cli_integration_tests", false);

    let temp_dir = TempDir::new()?;
    let cache_path = temp_dir.path().join("cache.json");

    // Create a peer address for testing
    let peer_addr: Multiaddr =
        "/ip4/127.0.0.1/udp/8080/quic-v1/p2p/12D3KooWRBhwfeP2Y4TCx1SM6s9rUoHhR5STiGwxBhgFRcw3UERE"
            .parse()?;

    let config = BootstrapCacheConfig::empty().with_cache_path(&cache_path);

    // Create args with peers but no test network mode
    let args = PeersArgs {
        first: false,
        addrs: vec![peer_addr.clone()],
        network_contacts_url: None,
        local: false,
        disable_mainnet_contacts: false,
        ignore_cache: false,
    };

    let mut store = BootstrapCacheStore::empty(config.clone())?;
    store.initialize_from_peers_arg(&args).await?;
    let addrs = store.get_addrs().collect::<Vec<_>>();
    assert_eq!(addrs.len(), 1, "Should have one peer");
    assert_eq!(addrs[0].addr, peer_addr, "Should have the correct peer");

    // Verify cache was updated
    assert!(cache_path.exists(), "Cache file should exist");
    let cache_contents = fs::read_to_string(&cache_path)?;
    assert!(
        cache_contents.contains(&peer_addr.to_string()),
        "Cache should contain the peer address"
    );

    Ok(())
}
