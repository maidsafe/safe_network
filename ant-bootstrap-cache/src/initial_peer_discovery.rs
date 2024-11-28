// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{craft_valid_multiaddr_from_str, BootstrapEndpoints, BootstrapPeer, Error, Result};
use futures::stream::{self, StreamExt};
use reqwest::Client;
use std::time::Duration;
use url::Url;

/// The default network contacts endpoint
const DEFAULT_BOOTSTRAP_ENDPOINT: &str =
    "https://sn-testnet.s3.eu-west-2.amazonaws.com/network-contacts";
/// The client fetch timeout
const FETCH_TIMEOUT_SECS: u64 = 30;
/// Maximum number of endpoints to fetch at a time
const MAX_CONCURRENT_FETCHES: usize = 3;
/// The max number of retries for a endpoint on failure.
const MAX_RETRIES_ON_FETCH_FAILURE: usize = 3;

/// Discovers initial peers from a list of endpoints
pub struct InitialPeerDiscovery {
    /// The list of endpoints
    endpoints: Vec<Url>,
    /// Reqwest Client
    request_client: Client,
}

impl InitialPeerDiscovery {
    /// Create a new struct with the default endpoint
    pub fn new() -> Result<Self> {
        Self::with_endpoints(vec![DEFAULT_BOOTSTRAP_ENDPOINT
            .parse()
            .expect("Invalid URL")])
    }

    /// Create a new struct with the provided endpoints
    pub fn with_endpoints(endpoints: Vec<Url>) -> Result<Self> {
        #[cfg(not(target_arch = "wasm32"))]
        let request_client = Client::builder()
            .timeout(Duration::from_secs(FETCH_TIMEOUT_SECS))
            .build()?;
        // Wasm does not have the timeout method yet.
        #[cfg(target_arch = "wasm32")]
        let request_client = Client::builder().build()?;

        Ok(Self {
            endpoints,
            request_client,
        })
    }

    /// Fetch peers from all configured endpoints
    pub async fn fetch_peers(&self) -> Result<Vec<BootstrapPeer>> {
        info!(
            "Starting peer discovery from {} endpoints: {:?}",
            self.endpoints.len(),
            self.endpoints
        );
        let mut peers = Vec::new();
        let mut last_error = None;

        let mut fetches = stream::iter(self.endpoints.clone())
            .map(|endpoint| async move {
                info!("Attempting to fetch peers from endpoint: {}", endpoint);
                (
                    Self::fetch_from_endpoint(self.request_client.clone(), &endpoint).await,
                    endpoint,
                )
            })
            .buffer_unordered(MAX_CONCURRENT_FETCHES);

        while let Some((result, endpoint)) = fetches.next().await {
            match result {
                Ok(mut endpoint_peers) => {
                    info!(
                        "Successfully fetched {} peers from {}. First few peers: {:?}",
                        endpoint_peers.len(),
                        endpoint,
                        endpoint_peers.iter().take(3).collect::<Vec<_>>()
                    );
                    peers.append(&mut endpoint_peers);
                }
                Err(e) => {
                    warn!("Failed to fetch peers from {}: {}", endpoint, e);
                    last_error = Some(e);
                }
            }
        }

        if peers.is_empty() {
            last_error.map_or_else(
                || {
                    warn!("No peers found from any endpoint and no errors reported");
                    Err(Error::NoPeersFound(
                        "No valid peers found from any endpoint".to_string(),
                    ))
                },
                |e| {
                    warn!("No peers found from any endpoint. Last error: {}", e);
                    Err(Error::NoPeersFound(format!(
                        "No valid peers found from any endpoint: {e}",
                    )))
                },
            )
        } else {
            info!(
                "Successfully discovered {} total peers. First few: {:?}",
                peers.len(),
                peers.iter().take(3).collect::<Vec<_>>()
            );
            Ok(peers)
        }
    }

    /// Fetch the list of bootstrap peer from a single endpoint
    async fn fetch_from_endpoint(
        request_client: Client,
        endpoint: &Url,
    ) -> Result<Vec<BootstrapPeer>> {
        info!("Fetching peers from endpoint: {endpoint}");
        let mut retries = 0;

        let peers = loop {
            let response = request_client.get(endpoint.clone()).send().await;

            match response {
                Ok(response) => {
                    if response.status().is_success() {
                        let text = response.text().await?;

                        match Self::try_parse_response(&text) {
                            Ok(peers) => break peers,
                            Err(err) => {
                                warn!("Failed to parse response with err: {err:?}");
                                retries += 1;
                                if retries >= MAX_RETRIES_ON_FETCH_FAILURE {
                                    return Err(Error::FailedToObtainPeersFromUrl(
                                        endpoint.to_string(),
                                        MAX_RETRIES_ON_FETCH_FAILURE,
                                    ));
                                }
                            }
                        }
                    } else {
                        retries += 1;
                        if retries >= MAX_RETRIES_ON_FETCH_FAILURE {
                            return Err(Error::FailedToObtainPeersFromUrl(
                                endpoint.to_string(),
                                MAX_RETRIES_ON_FETCH_FAILURE,
                            ));
                        }
                    }
                }
                Err(err) => {
                    error!("Failed to get peers from URL {endpoint}: {err:?}");
                    retries += 1;
                    if retries >= MAX_RETRIES_ON_FETCH_FAILURE {
                        return Err(Error::FailedToObtainPeersFromUrl(
                            endpoint.to_string(),
                            MAX_RETRIES_ON_FETCH_FAILURE,
                        ));
                    }
                }
            }
            trace!(
                "Failed to get peers from URL, retrying {retries}/{MAX_RETRIES_ON_FETCH_FAILURE}"
            );
            tokio::time::sleep(Duration::from_secs(1)).await;
        };

        Ok(peers)
    }

    /// Try to parse a response from a endpoint
    fn try_parse_response(response: &str) -> Result<Vec<BootstrapPeer>> {
        match serde_json::from_str::<BootstrapEndpoints>(response) {
            Ok(json_endpoints) => {
                info!(
                    "Successfully parsed JSON response with {} peers",
                    json_endpoints.peers.len()
                );
                let peers = json_endpoints
                    .peers
                    .into_iter()
                    .filter_map(|addr_str| craft_valid_multiaddr_from_str(&addr_str))
                    .map(BootstrapPeer::new)
                    .collect::<Vec<_>>();

                if peers.is_empty() {
                    warn!("No valid peers found in JSON response");
                    Err(Error::NoPeersFound(
                        "No valid peers found in JSON response".to_string(),
                    ))
                } else {
                    info!("Successfully parsed {} valid peers from JSON", peers.len());
                    Ok(peers)
                }
            }
            Err(e) => {
                info!("Attempting to parse response as plain text");
                // Try parsing as plain text with one multiaddr per line
                // example of contacts file exists in resources/network-contacts-examples
                let peers = response
                    .split('\n')
                    .filter_map(craft_valid_multiaddr_from_str)
                    .map(BootstrapPeer::new)
                    .collect::<Vec<_>>();

                if peers.is_empty() {
                    warn!(
                        "No valid peers found in plain text response. Previous Json error: {e:?}"
                    );
                    Err(Error::NoPeersFound(
                        "No valid peers found in plain text response".to_string(),
                    ))
                } else {
                    info!(
                        "Successfully parsed {} valid peers from plain text",
                        peers.len()
                    );
                    Ok(peers)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::Multiaddr;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    #[tokio::test]
    async fn test_fetch_peers() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("/ip4/127.0.0.1/tcp/8080\n/ip4/127.0.0.2/tcp/8080"),
            )
            .mount(&mock_server)
            .await;

        let mut discovery = InitialPeerDiscovery::new().unwrap();
        discovery.endpoints = vec![mock_server.uri().parse().unwrap()];

        let peers = discovery.fetch_peers().await.unwrap();
        assert_eq!(peers.len(), 2);

        let addr1: Multiaddr = "/ip4/127.0.0.1/tcp/8080".parse().unwrap();
        let addr2: Multiaddr = "/ip4/127.0.0.2/tcp/8080".parse().unwrap();
        assert!(peers.iter().any(|p| p.addr == addr1));
        assert!(peers.iter().any(|p| p.addr == addr2));
    }

    #[tokio::test]
    async fn test_endpoint_failover() {
        let mock_server1 = MockServer::start().await;
        let mock_server2 = MockServer::start().await;

        // First endpoint fails
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server1)
            .await;

        // Second endpoint succeeds
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_string("/ip4/127.0.0.1/tcp/8080"))
            .mount(&mock_server2)
            .await;

        let mut discovery = InitialPeerDiscovery::new().unwrap();
        discovery.endpoints = vec![
            mock_server1.uri().parse().unwrap(),
            mock_server2.uri().parse().unwrap(),
        ];

        let peers = discovery.fetch_peers().await.unwrap();
        assert_eq!(peers.len(), 1);

        let addr: Multiaddr = "/ip4/127.0.0.1/tcp/8080".parse().unwrap();
        assert_eq!(peers[0].addr, addr);
    }

    #[tokio::test]
    async fn test_invalid_multiaddr() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(
                    "/ip4/127.0.0.1/tcp/8080\ninvalid-addr\n/ip4/127.0.0.2/tcp/8080",
                ),
            )
            .mount(&mock_server)
            .await;

        let mut discovery = InitialPeerDiscovery::new().unwrap();
        discovery.endpoints = vec![mock_server.uri().parse().unwrap()];

        let peers = discovery.fetch_peers().await.unwrap();
        let valid_addr: Multiaddr = "/ip4/127.0.0.1/tcp/8080".parse().unwrap();
        assert_eq!(peers[0].addr, valid_addr);
    }

    #[tokio::test]
    async fn test_empty_response() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_string(""))
            .mount(&mock_server)
            .await;

        let mut discovery = InitialPeerDiscovery::new().unwrap();
        discovery.endpoints = vec![mock_server.uri().parse().unwrap()];

        let result = discovery.fetch_peers().await;

        assert!(matches!(result, Err(Error::NoPeersFound(_))));
    }

    #[tokio::test]
    async fn test_whitespace_and_empty_lines() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string("\n  \n/ip4/127.0.0.1/tcp/8080\n  \n"),
            )
            .mount(&mock_server)
            .await;

        let mut discovery = InitialPeerDiscovery::new().unwrap();
        discovery.endpoints = vec![mock_server.uri().parse().unwrap()];

        let peers = discovery.fetch_peers().await.unwrap();
        assert_eq!(peers.len(), 1);

        let addr: Multiaddr = "/ip4/127.0.0.1/tcp/8080".parse().unwrap();
        assert_eq!(peers[0].addr, addr);
    }

    #[tokio::test]
    async fn test_default_endpoints() {
        let discovery = InitialPeerDiscovery::new().unwrap();
        assert_eq!(discovery.endpoints.len(), 1);
        assert_eq!(
            discovery.endpoints[0],
            "https://sn-testnet.s3.eu-west-2.amazonaws.com/network-contacts"
                .parse()
                .unwrap()
        );
    }

    #[tokio::test]
    async fn test_custom_endpoints() {
        let endpoints = vec!["http://example.com".parse().unwrap()];
        let discovery = InitialPeerDiscovery::with_endpoints(endpoints.clone()).unwrap();
        assert_eq!(discovery.endpoints, endpoints);
    }

    #[tokio::test]
    async fn test_json_endpoints() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"peers": ["/ip4/127.0.0.1/tcp/8080", "/ip4/127.0.0.2/tcp/8080"]}"#,
            ))
            .mount(&mock_server)
            .await;

        let mut discovery = InitialPeerDiscovery::new().unwrap();
        discovery.endpoints = vec![mock_server.uri().parse().unwrap()];

        let peers = discovery.fetch_peers().await.unwrap();
        assert_eq!(peers.len(), 2);

        let addr1: Multiaddr = "/ip4/127.0.0.1/tcp/8080".parse().unwrap();
        let addr2: Multiaddr = "/ip4/127.0.0.2/tcp/8080".parse().unwrap();
        assert!(peers.iter().any(|p| p.addr == addr1));
        assert!(peers.iter().any(|p| p.addr == addr2));
    }
}
