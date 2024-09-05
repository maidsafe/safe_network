//! Connect to and build on the Autonomi network.
//!
//! # Examples
//!
//! ```no_run
//! # use libautonomi::Client;
//! let peers = ["/ip4/127.0.0.1/udp/1234/quic-v1".parse().expect("str to be valid multiaddr")];
//! let client = Client::connect(&peers).await?;
//! ```
//!
//! # Features
//!
//! - `local`: Enables local discovery of peers using mDNS.

#[doc(no_inline)] // Place this under 'Re-exports' in the docs.
pub use bytes::Bytes;
#[doc(no_inline)] // Place this under 'Re-exports' in the docs.
pub use libp2p::Multiaddr;

pub use client::Client;
pub use client::ConnectError;

mod client;
mod secrets;
mod self_encryption;
mod wallet;

const VERIFY_STORE: bool = true;
