// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod api;
mod error;
mod event;

pub use self::event::NodeEvent;

use self::{error::Error, event::NodeEventsChannel};

use crate::{
    network::Network,
    storage::{ChunkStorage, RegisterStorage},
    transfers::Transfers,
};

use libp2p::PeerId;
use xor_name::{XorName, XOR_NAME_LEN};

/// `Node` represents a single node in the distributed network. It handles
/// network events, processes incoming requests, interacts with the data
/// storage, and broadcasts node-related events.
// #[derive(Clone)]
pub struct Node {
    network: Network,
    chunks: ChunkStorage,
    registers: RegisterStorage,
    transfers: Transfers,
    events_channel: NodeEventsChannel,
}

/// Returns a an `XorName` representation of the `PeerId`.
pub fn to_xorname(peer_id: PeerId) -> XorName {
    let mut xorname_bytes = [0u8; XOR_NAME_LEN];
    let peer_id_bytes = peer_id.to_bytes();
    xorname_bytes.copy_from_slice(&peer_id_bytes[0..32]);
    XorName(xorname_bytes)
}
