// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::error::{Error, Result};
use bls::PublicKey;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use sn_protocol::storage::{ChunkAddress, RegisterAddress};
use sn_transfers::{CashNoteRedemption, UniquePubkey};
use tokio::sync::broadcast;

const NODE_EVENT_CHANNEL_SIZE: usize = 500;

/// Channel where users of the public API can listen to events broadcasted by the node.
#[derive(Clone)]
pub struct NodeEventsChannel(broadcast::Sender<NodeEvent>);

/// Type of channel receiver where events are broadcasted to by the node.
pub type NodeEventsReceiver = broadcast::Receiver<NodeEvent>;

impl Default for NodeEventsChannel {
    fn default() -> Self {
        Self(broadcast::channel(NODE_EVENT_CHANNEL_SIZE).0)
    }
}

impl NodeEventsChannel {
    /// Returns a new receiver to listen to the channel.
    /// Multiple receivers can be actively listening.
    pub fn subscribe(&self) -> broadcast::Receiver<NodeEvent> {
        self.0.subscribe()
    }

    // Broadcast a new event, meant to be a helper only used by the sn_node's internals.
    pub(crate) fn broadcast(&self, event: NodeEvent) {
        let event_string = format!("{:?}", event);
        if let Err(err) = self.0.send(event) {
            trace!(
                "Error occurred when trying to broadcast a node event ({event_string:?}): {err}"
            );
        }
    }

    /// Returns the number of active receivers
    pub fn receiver_count(&self) -> usize {
        self.0.receiver_count()
    }
}

/// Type of events broadcasted by the node to the public API.
#[derive(Clone, Serialize, custom_debug::Debug, Deserialize)]
pub enum NodeEvent {
    /// The node has been connected to the network
    ConnectedToNetwork,
    /// A Chunk has been stored in local storage
    ChunkStored(ChunkAddress),
    /// A Register has been created in local storage
    RegisterCreated(RegisterAddress),
    /// A Register edit operation has been applied in local storage
    RegisterEdited(RegisterAddress),
    /// A CashNote Spend has been stored in local storage
    SpendStored(UniquePubkey),
    /// One of the sub event channel closed and unrecoverable.
    ChannelClosed,
    /// AutoNAT discovered we are behind a NAT, thus private.
    BehindNat,
    /// Gossipsub message received
    GossipsubMsg {
        /// Topic the message was published on
        topic: String,
        /// The raw bytes of the received message
        #[debug(skip)]
        msg: Bytes,
    },
    /// Transfer notification message received for a public key
    TransferNotif {
        /// Public key the transfer notification is about
        key: PublicKey,
        /// The cashnote redemptions of the transfers
        cashnote_redemptions: Vec<CashNoteRedemption>,
    },
}

impl NodeEvent {
    /// Convert NodeEvent to bytes
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        rmp_serde::to_vec(&self).map_err(|_| Error::NodeEventParsingFailed)
    }

    /// Get NodeEvent from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        rmp_serde::from_slice(bytes).map_err(|_| Error::NodeEventParsingFailed)
    }
}
