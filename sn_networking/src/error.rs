// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{cmd::SwarmCmd, NetworkEvent};

use bytes::Bytes;
use libp2p::{
    kad,
    request_response::{OutboundFailure, RequestId},
    swarm::DialError,
    TransportError,
};
use sn_protocol::messages::Response;
use std::{io, path::PathBuf};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

pub(super) type Result<T, E = Error> = std::result::Result<T, E>;

/// Internal error.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error("Close group size must be a non-zero usize")]
    InvalidCloseGroupSize,

    #[error("Internal messaging channel was dropped")]
    InternalMsgChannelDropped,

    #[error("Response received for a request not found in our local tracking map: {0}")]
    ReceivedResponseDropped(RequestId),

    #[error("Outgoing response has been dropped due to a conn being closed or timeout: {0}")]
    OutgoingResponseDropped(Response),

    #[error("Could not retrieve the record after storing it: {0:}")]
    FailedToVerifyRecordWasStored(PrettyPrintRecordKey),

    #[error("Record retrieved from the network does not match the one we attempted to store {0:}")]
    ReturnedRecordDoesNotMatch(PrettyPrintRecordKey),

    #[error("Could not create storage dir: {path:?}, error: {source}")]
    FailedToCreateRecordStoreDir {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Transport Error")]
    TransportError(#[from] TransportError<std::io::Error>),

    #[error("Dial Error")]
    DialError(#[from] DialError),

    #[error("This peer is already being dialed: {0}")]
    AlreadyDialingPeer(libp2p::PeerId),

    #[error("Outbound Error")]
    OutboundError(#[from] OutboundFailure),

    #[error("Kademlia Store error: {0}")]
    KademliaStoreError(#[from] kad::store::Error),

    #[error("The mpsc::receiver for `NetworkEvent` has been dropped")]
    NetworkEventReceiverDropped(#[from] mpsc::error::SendError<NetworkEvent>),

    #[error("A Kademlia event has been dropped: {0:?}")]
    ReceivedKademliaEventDropped(kad::KademliaEvent),

    #[error("The mpsc::receiver for `SwarmCmd` has been dropped")]
    SwarmCmdReceiverDropped(#[from] mpsc::error::SendError<SwarmCmd>),

    #[error("The oneshot::sender has been dropped")]
    SenderDropped(#[from] oneshot::error::RecvError),

    #[error("Could not get enough peers ({required}) to satisfy the request, found {found}")]
    NotEnoughPeers { found: usize, required: usize },

    #[error("Record was not found locally")]
    RecordNotFound,

    #[error("Error putting record")]
    PutRecordError(#[from] kad::PutRecordError),

    #[error("No SwarmCmd channel capacity")]
    NoSwarmCmdChannelCapacity,
}

/// Pretty print a `kad::RecordKey` as a hex string.
/// So clients can use the hex string for xorname and record keys interchangeably.
/// This makes errors actionable for clients.
/// The only cost is converting ked::RecordKey into it before sending it in errors: `record_key.into()`
#[derive(Clone)]
pub struct PrettyPrintRecordKey(kad::RecordKey);

// seamless conversion from `kad::RecordKey` to `PrettyPrintRecordKey`
impl From<kad::RecordKey> for PrettyPrintRecordKey {
    fn from(key: kad::RecordKey) -> Self {
        PrettyPrintRecordKey(key)
    }
}

impl std::fmt::Display for PrettyPrintRecordKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let b: Vec<u8> = self.0.as_ref().to_vec();
        let record_key_b = Bytes::from(b);
        write!(f, "{:64x}", record_key_b)
    }
}

impl std::fmt::Debug for PrettyPrintRecordKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

#[cfg(test)]
mod tests {
    use sn_protocol::{storage::ChunkAddress, NetworkAddress};
    use xor_name::XorName;

    use super::*;

    #[test]
    fn test_client_sees_same_hex_in_errors_for_xorname_and_record_keys() {
        let mut rng = rand::thread_rng();
        let xor_name = XorName::random(&mut rng);
        let address = ChunkAddress::new(xor_name);
        let record_key = NetworkAddress::from_chunk_address(address).to_record_key();
        let pretty_record: PrettyPrintRecordKey = record_key.into();
        let record_str = format!("{}", pretty_record);
        let xor_name_str = format!("{:64x}", xor_name);
        println!("record_str: {}", record_str);
        println!("xor_name_str: {}", xor_name_str);
        assert_eq!(record_str, xor_name_str);
    }
}
