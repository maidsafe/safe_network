// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use libp2p::{
    gossipsub::{PublishError, SubscriptionError},
    kad::{self, Record},
    request_response::{OutboundFailure, OutboundRequestId},
    swarm::DialError,
    PeerId, TransportError,
};
use sn_protocol::{messages::Response, PrettyPrintRecordKey};
use std::{
    collections::{HashMap, HashSet},
    io,
    path::PathBuf,
};
use thiserror::Error;
use tokio::sync::oneshot;
use xor_name::XorName;

pub(super) type Result<T, E = Error> = std::result::Result<T, E>;

/// Internal error.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error("Network query timed out")]
    QueryTimeout,

    #[error("Close group size must be a non-zero usize")]
    InvalidCloseGroupSize,

    #[error("Could ont build the gossipsub config: {0}")]
    GossipsubConfigError(String),

    #[error("Internal messaging channel was dropped")]
    InternalMsgChannelDropped,

    #[error("Response received for a request not found in our local tracking map: {0}")]
    ReceivedResponseDropped(OutboundRequestId),

    #[error("Outgoing response has been dropped due to a conn being closed or timeout: {0}")]
    OutgoingResponseDropped(Response),

    /// No put_record attempts were successfully verified.
    #[error("Could not retrieve the record after storing it: {0:}")]
    FailedToVerifyRecordWasStored(PrettyPrintRecordKey<'static>),

    #[error("Record retrieved from the network does not match the one we attempted to store {0:}")]
    ReturnedRecordDoesNotMatch(PrettyPrintRecordKey<'static>),

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

    #[error("Outbound Error")]
    OutboundError(#[from] OutboundFailure),

    #[error("Kademlia Store error: {0}")]
    KademliaStoreError(#[from] kad::store::Error),

    #[error("A Kademlia event has been dropped: {0:?}")]
    ReceivedKademliaEventDropped(kad::Event),

    #[error("The oneshot::sender has been dropped")]
    SenderDropped(#[from] oneshot::error::RecvError),

    #[error("Could not get enough peers ({required}) to satisfy the request, found {found}")]
    NotEnoughPeers { found: usize, required: usize },

    #[error("Record was not found locally")]
    RecordNotFound,

    #[error("Get Record completed with non enough copies")]
    RecordNotEnoughCopies(Record),

    #[error("SnProtocol Error")]
    ProtocolError(#[from] sn_protocol::error::Error),

    #[error("No SwarmCmd channel capacity")]
    NoSwarmCmdChannelCapacity,

    #[error("Failed to sign the message with the PeerId keypair")]
    SigningFailed(#[from] libp2p::identity::SigningError),

    #[error("Failed to pop from front of CircularVec")]
    CircularVecPopFrontError,

    #[error("Node Listen Address was not provided during construction")]
    ListenAddressNotProvided,

    #[cfg(feature = "open-metrics")]
    #[error("Network Metric error")]
    NetworkMetricError,

    #[error("Gossipsub publish Error: {0}")]
    GossipsubPublishError(#[from] PublishError),

    #[error("Gossipsub subscribe Error: {0}")]
    GossipsubSubscriptionError(#[from] SubscriptionError),

    // Avoid logging the whole `Record` content by accident
    #[error("Split Record has {} different copies", result_map.len())]
    SplitRecord {
        result_map: HashMap<XorName, (Record, HashSet<PeerId>)>,
    },

    #[error("Record header is incorrect")]
    InCorrectRecordHeader,
}

#[cfg(test)]
mod tests {
    use sn_protocol::{storage::ChunkAddress, NetworkAddress, PrettyPrintKBucketKey};
    use xor_name::XorName;

    use super::*;

    #[test]
    fn test_client_sees_same_hex_in_errors_for_xorname_and_record_keys() {
        let mut rng = rand::thread_rng();
        let xor_name = XorName::random(&mut rng);
        let address = ChunkAddress::new(xor_name);
        let network_address = NetworkAddress::from_chunk_address(address);
        let record_key = network_address.to_record_key();
        let record_str = format!("{}", PrettyPrintRecordKey::from(&record_key));
        let xor_name_str = format!(
            "{:64x}({:?})",
            xor_name,
            PrettyPrintKBucketKey(network_address.as_kbucket_key())
        );
        println!("record_str: {}", record_str);
        println!("xor_name_str: {}", xor_name_str);
        assert_eq!(record_str, xor_name_str);
    }
}
