// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use futures::channel::{mpsc, oneshot};
use libp2p::{kad, request_response::OutboundFailure, swarm::DialError, TransportError};
use std::io;
use thiserror::Error;

/// The type returned by the `sn_routing` message handling methods.
pub(super) type Result<T, E = Error> = std::result::Result<T, E>;

/// Internal error.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error("Othe error: {0}")]
    Other(String),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Transport Error")]
    TransportError(#[from] TransportError<std::io::Error>),

    #[error("Dial Error")]
    DialError(#[from] DialError),

    #[error("Outbound Error")]
    OutboundError(#[from] OutboundFailure),

    #[error("Kademlia Store error: {0}")]
    KademliaStoreError(#[from] kad::store::Error),

    #[error("The mpsc::receiever has been dropped")]
    ReceieverDropped(#[from] mpsc::SendError),

    #[error("The oneshot::sender has been dropped")]
    SenderDropped(#[from] oneshot::Canceled),
}
