//! Connect to and build on the Autonomi network.
//!
//! # Data types
//!
//! This API gives access to two fundamental types on the network: chunks and
//! registers.
//!
//! When we upload data, it's split into chunks using self-encryption, yielding
//! a 'data map' allowing us to reconstruct the data again. Any two people that
//! upload the exact same data will get the same data map, as all chunks are
//! content-addressed and self-encryption is deterministic.
//!
//! Registers can keep small values pointing to data. This value can be updated
//! and the history is kept. Multiple values can exist side by side in case of
//! concurrency, but should converge to a single value eventually.
//!
//! # Example
//!
//! ```no_run
//! # use autonomi::{Client, Bytes};
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let peers = ["/ip4/127.0.0.1/udp/1234/quic-v1".parse()?];
//! let client = Client::connect(&peers).await?;
//!
//! # let mut wallet = todo!();
//! let addr = client.put(Bytes::from("Hello, World"), &mut wallet).await?;
//! let data = client.get(addr).await?;
//! assert_eq!(data, Bytes::from("Hello, World"));
//! # Ok(())
//! # }
//! ```
//!
//! # Features
//!
//! - `local`: Discover local peers using mDNS. Useful for development.

#[doc(no_inline)] // Place this under 'Re-exports' in the docs.
pub use bytes::Bytes;
#[doc(no_inline)] // Place this under 'Re-exports' in the docs.
pub use libp2p::Multiaddr;

pub use client::{Client, ConnectError, CONNECT_TIMEOUT_SECS};

mod client;
mod secrets;
mod self_encryption;
mod wallet;

const VERIFY_STORE: bool = true;
