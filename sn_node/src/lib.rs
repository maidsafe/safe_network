// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

//! Implementation of the Node in SAFE Network.

// For quick_error
#![recursion_limit = "256"]
#![doc(
    html_logo_url = "https://github.com/maidsafe/QA/raw/master/Images/maidsafe_logo.png",
    html_favicon_url = "https://maidsafe.net/img/favicon.ico",
    test(attr(deny(warnings)))
)]
// Forbid some very bad patterns. Forbid is stronger than `deny`, preventing us from suppressing the
// lint with `#[allow(...)]` et-all.
#![forbid(
    arithmetic_overflow,
    mutable_transmutes,
    no_mangle_const_items,
    unknown_crate_types,
    unsafe_code
)]
// Turn on some additional warnings to encourage good style.
#![warn(
    missing_docs,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    unused_extern_crates,
    unused_import_braces,
    unused_qualifications,
    unused_results,
    clippy::unicode_not_nfc,
    clippy::unwrap_used,
    clippy::unused_async
)]
#![allow(clippy::result_large_err)]

#[macro_use]
extern crate tracing;

mod api;
mod error;
mod event;
mod spendbook;

use spendbook::SpendBook;

pub use self::{
    api::RunningNode,
    event::{NodeEvent, NodeEventsChannel, NodeEventsReceiver},
};

use libp2p::{Multiaddr, PeerId};
use sn_networking::Network;
use sn_registers::RegisterStorage;

/// `Node` represents a single node in the distributed network. It handles
/// network events, processes incoming requests, interacts with the data
/// storage, and broadcasts node-related events.
pub struct Node {
    network: Network,
    registers: RegisterStorage,
    spendbook: SpendBook,
    events_channel: NodeEventsChannel,
    /// Peers that are dialed at startup of node.
    initial_peers: Vec<(PeerId, Multiaddr)>,
}
