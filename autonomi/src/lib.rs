// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

//! Connect to and build on the Autonomi network.
//!
//! # Example
//!
//! ```rust
//! use autonomi::{Bytes, Client, Wallet};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let client = Client::connect(&["/ip4/127.0.0.1/udp/1234/quic-v1".parse()?]).await?;
//!
//!     // Default wallet of testnet.
//!     let key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
//!     let wallet = Wallet::new_from_private_key(Default::default(), key)?;
//!
//!     // Put and fetch data.
//!     let data_addr = client.data_put_public(Bytes::from("Hello, World"), (&wallet).into()).await?;
//!     let _data_fetched = client.data_get_public(data_addr).await?;
//!
//!     // Put and fetch directory from local file system.
//!     let dir_addr = client.dir_upload_public("files/to/upload".into(), &wallet).await?;
//!     client.dir_download_public(dir_addr, "files/downloaded".into()).await?;
//!
//!     Ok(())
//! }
//! ```
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
//! - `fs`: Up/download files and directories from filesystem
//! - `registers`: Operate on register datatype
//! - `vault`: Operate on Vault datatype
//! - `full`: All of above
//! - `local`: Discover local peers using mDNS. Useful for development.
//! - `loud`: Print debug information to stdout

// docs.rs generation will enable unstable `doc_cfg` feature
#![cfg_attr(docsrs, feature(doc_cfg))]

#[macro_use]
extern crate tracing;

pub mod client;
mod self_encryption;

pub use ant_evm::get_evm_network_from_env;
pub use ant_evm::EvmNetwork as Network;
pub use ant_evm::EvmWallet as Wallet;
pub use ant_evm::RewardsAddress;
#[cfg(feature = "external-signer")]
pub use utils::receipt_from_quotes_and_payments;

#[doc(no_inline)] // Place this under 'Re-exports' in the docs.
pub use bytes::Bytes;
#[doc(no_inline)] // Place this under 'Re-exports' in the docs.
pub use libp2p::Multiaddr;

#[doc(inline)]
pub use client::{files::archive::PrivateArchive, Client};

#[cfg(feature = "extension-module")]
mod python;
mod utils;
