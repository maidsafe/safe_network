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

pub use self::{
    api::RunningNode,
    event::{NodeEvent, NodeEventsChannel, NodeEventsReceiver},
};

use self::api::TransferAction;

use crate::{
    domain::{node_transfers::Transfers, storage::RegisterStorage},
    network::Network,
};

use libp2p::{Multiaddr, PeerId};
use tokio::sync::mpsc;

/// `Node` represents a single node in the distributed network. It handles
/// network events, processes incoming requests, interacts with the data
/// storage, and broadcasts node-related events.
pub struct Node {
    network: Network,
    registers: RegisterStorage,
    transfers: Transfers,
    events_channel: NodeEventsChannel,
    /// Peers that are dialed at startup of node.
    initial_peers: Vec<(PeerId, Multiaddr)>,
    transfer_actor: mpsc::Sender<TransferAction>,
}
