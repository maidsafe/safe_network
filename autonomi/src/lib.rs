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
//! # Features
//!
//! - `local`: Discover local peers using mDNS. Useful for development.

// docs.rs generation will enable unstable `doc_cfg` feature
#![cfg_attr(docsrs, feature(doc_cfg))]

#[doc(no_inline)] // Place this under 'Re-exports' in the docs.
pub use bytes::Bytes;
#[doc(no_inline)] // Place this under 'Re-exports' in the docs.
pub use libp2p::Multiaddr;

pub mod client;
#[cfg(feature = "evm-payments")]
pub mod evm;
#[cfg(feature = "native-payments")]
pub mod native;
mod self_encryption;

#[cfg(feature = "transfers")]
const VERIFY_STORE: bool = true;

#[cfg(all(feature = "native-payments", not(feature = "evm-payments")))]
pub type Client = native::Client;

#[cfg(all(feature = "evm-payments", not(feature = "native-payments")))]
pub type Client = evm::Client;
