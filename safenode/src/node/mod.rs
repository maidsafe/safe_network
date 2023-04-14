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

use crate::{network::Network, storage::DataStorage};

/// `Node` represents a single node in the distributed network. It handles
/// network events, processes incoming requests, interacts with the data
/// storage, and broadcasts node-related events.
#[derive(Clone)]
pub struct Node {
    /// The `Network` instance.
    pub network: Network,
    storage: DataStorage,
    events_channel: NodeEventsChannel,
}
