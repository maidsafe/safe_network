// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::error::Result;

use bytes::Bytes;
use serde::Serialize;
use tokio::sync::broadcast;

// Channel where events will be broadcasted by the client.
#[derive(Clone, Debug)]
pub(super) struct ClientEventsChannel(broadcast::Sender<ClientEvent>);

impl Default for ClientEventsChannel {
    fn default() -> Self {
        Self(broadcast::channel(100).0)
    }
}

impl ClientEventsChannel {
    /// Returns a new receiver to listen to the channel.
    /// Multiple receivers can be actively listening.
    pub(super) fn subscribe(&self) -> ClientEventsReceiver {
        ClientEventsReceiver(self.0.subscribe())
    }

    // Broadcast a new event, meant to be a helper only used by the client's internals.
    pub(crate) fn broadcast(&self, event: ClientEvent) -> Result<()> {
        let _subscriber_count = self.0.send(event)?;
        Ok(())
    }
}

/// Type of events broadcasted by the client to the public API.
#[derive(Clone, custom_debug::Debug, Serialize)]
pub enum ClientEvent {
    /// The client has been connected to the network
    ConnectedToNetwork,
    /// No network activity has been received for a given duration
    /// we should error out
    InactiveClient(std::time::Duration),
    /// Gossipsub message received on a topic the client has subscribed to
    GossipsubMsg {
        /// Topic the message was published on
        topic: String,
        /// The raw bytes of the received message
        #[debug(skip)]
        msg: Bytes,
    },
}

/// Receiver Channel where users of the public API can listen to events broadcasted by the client.
#[derive(Debug)]
pub struct ClientEventsReceiver(pub(super) broadcast::Receiver<ClientEvent>);

impl ClientEventsReceiver {
    /// Receive a new event, meant to be ued by the user of the public API.
    pub async fn recv(&mut self) -> Result<ClientEvent> {
        let event = self.0.recv().await?;
        Ok(event)
    }
}
