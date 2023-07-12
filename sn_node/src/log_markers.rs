// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use libp2p::PeerId;
use sn_protocol::messages::{Cmd, CmdResponse};
use std::time::Duration;
// this gets us to_string easily enough
use strum::Display;

/// Public Markers for generating log output,
/// These generate apprioriate log level output and consistent strings.
/// Changing these log markers is a breaking change.
#[derive(Debug, Clone, Display)]
pub enum Marker<'a> {
    /// The node has started
    NodeConnectedToNetwork,

    /// No network activity in some time
    NoNetworkActivity(Duration),

    /// Network Cmd message received
    NodeCmdReceived(&'a Cmd),

    /// Network Cmd message response was generated
    NodeCmdResponded(&'a CmdResponse),

    /// Peer was added to the routing table
    PeerAddedToRoutingTable(PeerId),

    /// Peer was removed from the routing table
    PeerRemovedFromRoutingTable(PeerId),

    /// Lost Record Detected
    LostRecordDetected(&'a Vec<PeerId>),

    /// Replication trigger was fired
    ReplicationTriggered((&'a PeerId, bool)),

    /// Keys of Records we are fetching to replicate locally
    FetchingKeysForReplication {
        /// fetching_keys_len: number of keys we are fetching
        fetching_keys_len: usize,
        /// provided_keys_len: number of keys we were provided (the difference would be the number of keys we already have)
        provided_keys_len: usize,
        /// peer_id: the peer we are fetching from
        peer_id: PeerId,
    },
}

impl<'a> Marker<'a> {
    /// Returns the string representation of the LogMarker.
    pub fn log(&self) {
        // Down the line, if some logs are noisier than others, we can
        // match the type and log a different level.
        info!("{self:?}");
    }
}
