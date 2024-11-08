// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

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
//! - `fs`: Up/download files and directories from filesystem
//! - `registers`: Operate on register datatype
//! - `data`: Operate on raw bytes and chunks
//! - `vault`: Operate on Vault datatype
//! - `full`: All of above
//! - `local`: Discover local peers using mDNS. Useful for development.
//! - `loud`: Print debug information to stdout

// docs.rs generation will enable unstable `doc_cfg` feature
#![cfg_attr(docsrs, feature(doc_cfg))]

#[macro_use]
extern crate tracing;

pub mod client;
#[cfg(feature = "data")]
mod self_encryption;
mod utils;

pub use sn_evm::get_evm_network_from_env;
pub use sn_evm::EvmNetwork;
pub use sn_evm::EvmWallet as Wallet;
pub use sn_evm::RewardsAddress;
#[cfg(feature = "external-signer")]
pub use utils::receipt_from_quotes_and_payments;

#[doc(no_inline)] // Place this under 'Re-exports' in the docs.
pub use bytes::Bytes;
#[doc(no_inline)] // Place this under 'Re-exports' in the docs.
pub use libp2p::Multiaddr;

pub use client::Client;

#[cfg(feature = "extension-module")]
mod python;
