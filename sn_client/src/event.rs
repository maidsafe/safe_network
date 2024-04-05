// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use serde::Serialize;
use tokio::sync::broadcast::{self, error::RecvError};

// Channel where events will be broadcasted by the client.
#[derive(Clone, Debug)]
pub struct ClientEventsBroadcaster(broadcast::Sender<ClientEvent>);

impl Default for ClientEventsBroadcaster {
    fn default() -> Self {
        Self(broadcast::channel(100).0)
    }
}

impl ClientEventsBroadcaster {
    /// Returns a new receiver to listen to the channel.
    /// Multiple receivers can be actively listening.
    pub fn subscribe(&self) -> ClientEventsReceiver {
        ClientEventsReceiver(self.0.subscribe())
    }

    // Broadcast a new event, meant to be a helper only used by the client's internals.
    pub(crate) fn broadcast(&self, event: ClientEvent) {
        if let Err(err) = self.0.send(event) {
            trace!(
                "Could not broadcast ClientEvent as we don't have any active listeners: {err:?}"
            );
        }
    }
}

/// Type of events broadcasted by the client to the public API.
#[derive(Clone, custom_debug::Debug, Serialize)]
pub enum ClientEvent {
    /// A peer has been added to the Routing table.
    /// Also contains the max number of peers to connect to before we receive ClientEvent::ConnectedToNetwork
    PeerAdded { max_peers_to_connect: usize },
    /// We've encountered a Peer with an unsupported protocol.
    PeerWithUnsupportedProtocol {
        our_protocol: String,
        their_protocol: String,
    },
    /// The client has been connected to the network
    ConnectedToNetwork,
    /// No network activity has been received for a given duration
    /// we should error out
    InactiveClient(tokio::time::Duration),
}

/// Receiver Channel where users of the public API can listen to events broadcasted by the client.
#[derive(Debug)]
pub struct ClientEventsReceiver(pub(super) broadcast::Receiver<ClientEvent>);

impl ClientEventsReceiver {
    /// Receive a new event, meant to be used by the user of the public API.
    pub async fn recv(&mut self) -> std::result::Result<ClientEvent, RecvError> {
        self.0.recv().await
    }
}
