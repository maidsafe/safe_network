// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{cmd::SwarmCmd, NetworkEvent};

use libp2p::{
    identity::DecodingError,
    kad::{self, Record},
    request_response::{OutboundFailure, RequestId},
    swarm::DialError,
    TransportError,
};
use sn_protocol::{messages::Response, PrettyPrintRecordKey};
use std::{io, path::PathBuf};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

pub(super) type Result<T, E = Error> = std::result::Result<T, E>;

/// Internal error.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error(
        "Not enough store cost quotes returned from the network to ensure a valid fee is paid"
    )]
    NotEnoughCostQuotes,
    #[error("No store cost returned from the network")]
    NoStoreCostReturned,

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
    #[error("Libp2p Idendity Decode Error")]
    LIbp2pDecode(#[from] DecodingError),

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

    #[error("Get Record completed with non enough copies")]
    RecordNotEnoughCopies(Record),

    #[error("Error putting record")]
    PutRecordError(#[from] kad::PutRecordError),

    #[error("No SwarmCmd channel capacity")]
    NoSwarmCmdChannelCapacity,

    #[error("Failed to sign the message with the PeerId keypair")]
    SigningFailed(#[from] libp2p::identity::SigningError),

    #[error("Failed to pop from front of CircularVec")]
    CircularVecPopFrontError,
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
