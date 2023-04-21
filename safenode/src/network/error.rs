// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{cmd::SwarmCmd, NetworkEvent};

use libp2p::{request_response::OutboundFailure, swarm::DialError, TransportError};
use serde::{Deserialize, Serialize};
use std::io;
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

pub(super) type Result<T, E = Error> = std::result::Result<T, E>;

/// Internal error.
#[derive(Debug, Error, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub enum Error {
    #[error("Internal messaging channel was dropped")]
    InternalMsgChannelDropped,

    #[error("Response received for a request not found in our local tracking map: {0}")]
    ReceivedResponseDropped(String),

    #[error("Outgoing response has been dropped due to a conn being closed or timeout: {0:?}")]
    OutgoingResponseDropped(String),

    #[error("I/O error: {0}")]
    Io(String),

    #[error("Transport Error")]
    TransportError(String),

    #[error("Dial Error")]
    DialError(String),

    #[error("Outbound Error")]
    OutboundError(String),

    #[error("The mpsc::receiver for `NetworkEvent` has been dropped")]
    NetworkEventReceiverDropped(String),

    #[error("A Kademlia event has been dropped: {0:?}")]
    ReceivedKademliaEventDropped(String),

    #[error("The mpsc::receiver for `SwarmCmd` has been dropped")]
    SwarmCmdReceiverDropped(String),

    #[error("Could not get CLOSE_GROUP_SIZE number of peers.")]
    NotEnoughPeers,

    #[error("ResponseTimeout")]
    ResponseTimeout(String),
}

impl From<mpsc::error::SendError<SwarmCmd>> for Error {
    fn from(e: mpsc::error::SendError<SwarmCmd>) -> Self {
        Self::SwarmCmdReceiverDropped(e.to_string())
    }
}

impl From<mpsc::error::SendError<NetworkEvent>> for Error {
    fn from(e: mpsc::error::SendError<NetworkEvent>) -> Self {
        Self::NetworkEventReceiverDropped(e.to_string())
    }
}

impl From<DialError> for Error {
    fn from(e: DialError) -> Self {
        Self::DialError(e.to_string())
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<TransportError<io::Error>> for Error {
    fn from(error: TransportError<io::Error>) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<oneshot::error::RecvError> for Error {
    fn from(error: oneshot::error::RecvError) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<OutboundFailure> for Error {
    fn from(e: OutboundFailure) -> Self {
        Self::OutboundError(e.to_string())
    }
}
