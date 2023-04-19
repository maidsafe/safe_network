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
    protocol::node_transfers::Transfers,
    storage::{ChunkStorage, RegisterStorage},
};

use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use xor_name::{XorName, XOR_NAME_LEN};

/// `Node` represents a single node in the distributed network. It handles
/// network events, processes incoming requests, interacts with the data
/// storage, and broadcasts node-related events.
pub struct Node {
    network: Network,
    chunks: ChunkStorage,
    registers: RegisterStorage,
    transfers: Transfers,
    events_channel: NodeEventsChannel,
}

/// A unique identifier for a node in the network,
/// by which we can know their location in the xor space.
#[derive(
    Copy, Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize,
)]
pub struct NodeId(XorName);

/// Returns a `NodeId` representation of the `PeerId`.
pub fn to_node_id(peer_id: PeerId) -> NodeId {
    let mut xorname_bytes = [0u8; XOR_NAME_LEN];
    let peer_id_bytes = peer_id.to_bytes();
    xorname_bytes.copy_from_slice(&peer_id_bytes[0..32]);
    NodeId(XorName(xorname_bytes))
}
