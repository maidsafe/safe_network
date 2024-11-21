# Bootstrap Cache

A decentralized peer discovery and caching system for the Safe Network.

## Features

- **Decentralized Design**: No dedicated bootstrap nodes required
- **Cross-Platform Support**: Works on Linux, macOS, and Windows
- **Shared Cache**: System-wide cache file accessible by both nodes and clients
- **Concurrent Access**: File locking for safe multi-process access
- **Atomic Operations**: Safe cache updates using atomic file operations
- **Initial Peer Discovery**: Fallback web endpoints for new/stale cache scenarios
- **Comprehensive Error Handling**: Detailed error types and logging
- **Circuit Breaker Pattern**: Intelligent failure handling with:
  - Configurable failure thresholds and reset timeouts
  - Exponential backoff for failed requests
  - Automatic state transitions (closed → open → half-open)
  - Protection against cascading failures

### Peer Management

The bootstrap cache implements a robust peer management system:

- **Peer Status Tracking**: Each peer's connection history is tracked, including:
  - Success count: Number of successful connections
  - Failure count: Number of failed connection attempts
  - Last seen timestamp: When the peer was last successfully contacted

- **Automatic Cleanup**: The system automatically removes unreliable peers:
  - Peers that fail 3 consecutive connection attempts are marked for removal
  - Removal only occurs if there are at least 2 working peers available
  - This ensures network connectivity is maintained even during temporary connection issues

- **Duplicate Prevention**: The cache automatically prevents duplicate peer entries:
  - Same IP and port combinations are only stored once
  - Different ports on the same IP are treated as separate peers

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
bootstrap_cache = { version = "0.1.0" }
```

## Usage

### Basic Example

```rust
use bootstrap_cache::{BootstrapCache, CacheManager, InitialPeerDiscovery};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the cache manager
    let cache_manager = CacheManager::new()?;

    // Try to read from the cache
    let mut cache = match cache_manager.read_cache() {
        Ok(cache) if !cache.is_stale() => cache,
        _ => {
            // Cache is stale or unavailable, fetch initial peers
            let discovery = InitialPeerDiscovery::new();
            let peers = discovery.fetch_peers().await?;
            let cache = BootstrapCache {
                last_updated: chrono::Utc::now(),
                peers,
            };
            cache_manager.write_cache(&cache)?;
            cache
        }
    };

    println!("Found {} peers in cache", cache.peers.len());
    Ok(())
}
```

### Custom Endpoints

```rust
use bootstrap_cache::InitialPeerDiscovery;

let discovery = InitialPeerDiscovery::with_endpoints(vec![
    "http://custom1.example.com/peers.json".to_string(),
    "http://custom2.example.com/peers.json".to_string(),
]);
```

### Circuit Breaker Configuration

```rust
use bootstrap_cache::{InitialPeerDiscovery, CircuitBreakerConfig};
use std::time::Duration;

// Create a custom circuit breaker configuration
let config = CircuitBreakerConfig {
    max_failures: 5,                            // Open after 5 failures
    reset_timeout: Duration::from_secs(300),    // Wait 5 minutes before recovery
    min_backoff: Duration::from_secs(1),        // Start with 1 second backoff
    max_backoff: Duration::from_secs(60),       // Max backoff of 60 seconds
};

// Initialize discovery with custom circuit breaker config
let discovery = InitialPeerDiscovery::with_config(config);
```

### Peer Management Example

```rust
use bootstrap_cache::BootstrapCache;

let mut cache = BootstrapCache::new();

// Add a new peer
cache.add_peer("192.168.1.1".to_string(), 8080);

// Update peer status after connection attempts
cache.update_peer_status("192.168.1.1", 8080, true);  // successful connection
cache.update_peer_status("192.168.1.1", 8080, false); // failed connection

// Clean up failed peers (only if we have at least 2 working peers)
cache.cleanup_failed_peers();
```

## Cache File Location

The cache file is stored in a system-wide location accessible to all processes:

- **Linux**: `/var/safe/bootstrap_cache.json`
- **macOS**: `/Library/Application Support/Safe/bootstrap_cache.json`
- **Windows**: `C:\ProgramData\Safe\bootstrap_cache.json`

## Cache File Format

```json
{
    "last_updated": "2024-02-20T15:30:00Z",
    "peers": [
        {
            "ip": "192.168.1.1",
            "port": 8080,
            "last_seen": "2024-02-20T15:30:00Z",
            "success_count": 10,
            "failure_count": 0
        }
    ]
}
```

## Error Handling

The crate provides detailed error types through the `Error` enum:

```rust
use bootstrap_cache::Error;

match cache_manager.read_cache() {
    Ok(cache) => println!("Cache loaded successfully"),
    Err(Error::CacheStale) => println!("Cache is stale"),
    Err(Error::CacheCorrupted) => println!("Cache file is corrupted"),
    Err(Error::Io(e)) => println!("IO error: {}", e),
    Err(e) => println!("Other error: {}", e),
}
```

## Thread Safety

The cache system uses file locking to ensure safe concurrent access:

- Shared locks for reading
- Exclusive locks for writing
- Atomic file updates using temporary files

## Development

### Building

```bash
cargo build
```

### Running Tests

```bash
cargo test
```

### Running with Logging

```rust
use tracing_subscriber::FmtSubscriber;

// Initialize logging
let subscriber = FmtSubscriber::builder()
    .with_max_level(tracing::Level::DEBUG)
    .init();
```

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -am 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the GPL-3.0 License - see the LICENSE file for details.

## Related Documentation

- [Bootstrap Cache PRD](docs/bootstrap_cache_prd.md)
- [Implementation Guide](docs/bootstrap_cache_implementation.md)
