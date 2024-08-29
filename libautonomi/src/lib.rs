// TODO: Remove this once we have a proper lib.rs
#![allow(dead_code)]

#[doc(no_inline)] // Place this under 'Re-exports' in the docs.
pub use bytes::Bytes;
pub use client::Client;
#[doc(no_inline)] // Place this under 'Re-exports' in the docs.
pub use libp2p::Multiaddr;

mod client;
mod client_wallet;
mod files;
mod secrets;
mod self_encryption;
mod wallet;

const VERIFY_STORE: bool = true;
