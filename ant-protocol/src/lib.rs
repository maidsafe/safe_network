// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[macro_use]
extern crate tracing;

/// Errors.
pub mod error;
/// Messages types
pub mod messages;
/// Helpers for antnode
pub mod node;
/// RPC commands to node
pub mod node_rpc;
/// Storage types for transactions, chunks and registers.
pub mod storage;
/// Network versioning
pub mod version;

// this includes code generated from .proto files
#[expect(clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#[cfg(feature = "rpc")]
pub mod antnode_proto {
    tonic::include_proto!("antnode_proto");
}
pub use error::Error;
use storage::ScratchpadAddress;

use self::storage::{ChunkAddress, LinkedListAddress, RegisterAddress};

/// Re-export of Bytes used throughout the protocol
pub use bytes::Bytes;

use ant_evm::U256;
use libp2p::{
    kad::{KBucketDistance as Distance, KBucketKey as Key, RecordKey},
    multiaddr::Protocol,
    Multiaddr, PeerId,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::str::FromStr;
use std::{
    borrow::Cow,
    fmt::{self, Debug, Display, Formatter, Write},
};
use xor_name::XorName;

/// The maximum number of peers to return in a `GetClosestPeers` response.
/// This is the group size used in safe network protocol to be responsible for
/// an item in the network.
/// The peer should be present among the CLOSE_GROUP_SIZE if we're fetching the close_group(peer)
/// The size has been set to 5 for improved performance.
pub const CLOSE_GROUP_SIZE: usize = 5;

/// Returns the UDP port from the provided MultiAddr.
pub fn get_port_from_multiaddr(multi_addr: &Multiaddr) -> Option<u16> {
    // assuming the listening addr contains /ip4/127.0.0.1/udp/56215/quic-v1/p2p/<peer_id>
    for protocol in multi_addr.iter() {
        if let Protocol::Udp(port) = protocol {
            return Some(port);
        }
    }
    None
}

// This conversion shall no longer be required once updated to the latest libp2p.
// Which can has the direct access to the Distance private field of U256.
pub fn convert_distance_to_u256(distance: &Distance) -> U256 {
    let addr_str = format!("{distance:?}");
    let numeric_part = addr_str
        .trim_start_matches("Distance(")
        .trim_end_matches(")")
        .to_string();
    let distance_value = U256::from_str(&numeric_part);
    distance_value.unwrap_or(U256::ZERO)
}

/// This is the address in the network by which proximity/distance
/// to other items (whether nodes or data chunks) are calculated.
///
/// This is the mapping from the XOR name used
/// by for example self encryption, or the libp2p `PeerId`,
/// to the key used in the Kademlia DHT.
/// All our xorname calculations shall be replaced with the `KBucketKey` calculations,
/// for getting proximity/distance to other items (whether nodes or data).
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum NetworkAddress {
    /// The NetworkAddress is representing a PeerId.
    PeerId(Bytes),
    /// The NetworkAddress is representing a ChunkAddress.
    ChunkAddress(ChunkAddress),
    /// The NetworkAddress is representing a TransactionAddress.
    TransactionAddress(LinkedListAddress),
    /// The NetworkAddress is representing a ChunkAddress.
    RegisterAddress(RegisterAddress),
    /// The NetworkAddress is representing a RecordKey.
    RecordKey(Bytes),
    /// The NetworkAddress is representing a ScratchpadAddress.
    ScratchpadAddress(ScratchpadAddress),
}

impl NetworkAddress {
    /// Return a `NetworkAddress` representation of the `ChunkAddress`.
    pub fn from_chunk_address(chunk_address: ChunkAddress) -> Self {
        NetworkAddress::ChunkAddress(chunk_address)
    }

    /// Return a `NetworkAddress` representation of the `TransactionAddress`.
    pub fn from_transaction_address(transaction_address: LinkedListAddress) -> Self {
        NetworkAddress::TransactionAddress(transaction_address)
    }
    /// Return a `NetworkAddress` representation of the `TransactionAddress`.
    pub fn from_scratchpad_address(address: ScratchpadAddress) -> Self {
        NetworkAddress::ScratchpadAddress(address)
    }

    /// Return a `NetworkAddress` representation of the `RegisterAddress`.
    pub fn from_register_address(register_address: RegisterAddress) -> Self {
        NetworkAddress::RegisterAddress(register_address)
    }

    /// Return a `NetworkAddress` representation of the `PeerId` by encapsulating its bytes.
    pub fn from_peer(peer_id: PeerId) -> Self {
        NetworkAddress::PeerId(Bytes::from(peer_id.to_bytes()))
    }

    /// Return a `NetworkAddress` representation of the `RecordKey` by encapsulating its bytes.
    pub fn from_record_key(record_key: &RecordKey) -> Self {
        NetworkAddress::RecordKey(Bytes::copy_from_slice(record_key.as_ref()))
    }

    /// Return the encapsulated bytes of this `NetworkAddress`.
    pub fn as_bytes(&self) -> Vec<u8> {
        match self {
            NetworkAddress::PeerId(bytes) | NetworkAddress::RecordKey(bytes) => bytes.to_vec(),
            NetworkAddress::ChunkAddress(chunk_address) => chunk_address.xorname().0.to_vec(),
            NetworkAddress::TransactionAddress(transaction_address) => {
                transaction_address.xorname().0.to_vec()
            }
            NetworkAddress::ScratchpadAddress(addr) => addr.xorname().0.to_vec(),
            NetworkAddress::RegisterAddress(register_address) => {
                register_address.xorname().0.to_vec()
            }
        }
    }

    /// Try to return the represented `PeerId`.
    pub fn as_peer_id(&self) -> Option<PeerId> {
        if let NetworkAddress::PeerId(bytes) = self {
            if let Ok(peer_id) = PeerId::from_bytes(bytes) {
                return Some(peer_id);
            }
        }

        None
    }

    /// Try to return the represented `XorName`.
    pub fn as_xorname(&self) -> Option<XorName> {
        match self {
            NetworkAddress::TransactionAddress(transaction_address) => {
                Some(*transaction_address.xorname())
            }
            NetworkAddress::ChunkAddress(chunk_address) => Some(*chunk_address.xorname()),
            NetworkAddress::RegisterAddress(register_address) => Some(register_address.xorname()),
            NetworkAddress::ScratchpadAddress(address) => Some(address.xorname()),
            _ => None,
        }
    }

    /// Try to return the represented `RecordKey`.
    pub fn as_record_key(&self) -> Option<RecordKey> {
        match self {
            NetworkAddress::RecordKey(bytes) => Some(RecordKey::new(bytes)),
            _ => None,
        }
    }

    /// Return the convertable `RecordKey`.
    pub fn to_record_key(&self) -> RecordKey {
        match self {
            NetworkAddress::RecordKey(bytes) => RecordKey::new(bytes),
            NetworkAddress::ChunkAddress(chunk_address) => RecordKey::new(chunk_address.xorname()),
            NetworkAddress::RegisterAddress(register_address) => {
                RecordKey::new(&register_address.xorname())
            }
            NetworkAddress::TransactionAddress(transaction_address) => {
                RecordKey::new(transaction_address.xorname())
            }
            NetworkAddress::ScratchpadAddress(addr) => RecordKey::new(&addr.xorname()),
            NetworkAddress::PeerId(bytes) => RecordKey::new(bytes),
        }
    }

    /// Return the `KBucketKey` representation of this `NetworkAddress`.
    ///
    /// The `KBucketKey` is used for calculating proximity/distance to other items (whether nodes or data).
    /// Important to note is that it will always SHA256 hash any bytes it receives.
    /// Therefore, the canonical use of distance/proximity calculations in the network
    /// is via the `KBucketKey`, or the convenience methods of `NetworkAddress`.
    pub fn as_kbucket_key(&self) -> Key<Vec<u8>> {
        Key::new(self.as_bytes())
    }

    /// Compute the distance of the keys according to the XOR metric.
    pub fn distance(&self, other: &NetworkAddress) -> Distance {
        self.as_kbucket_key().distance(&other.as_kbucket_key())
    }

    // NB: Leaving this here as to demonstrate what we can do with this.
    // /// Return the uniquely determined key with the given distance to `self`.
    // ///
    // /// This implements the following equivalence:
    // ///
    // /// `self xor other = distance <==> other = self xor distance`
    // pub fn for_distance(&self, d: Distance) -> libp2p::kad::kbucket::KeyBytes {
    //     self.as_kbucket_key().for_distance(d)
    // }
}

impl Debug for NetworkAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let name_str = match self {
            NetworkAddress::PeerId(_) => {
                if let Some(peer_id) = self.as_peer_id() {
                    format!("NetworkAddress::PeerId({peer_id} - ")
                } else {
                    "NetworkAddress::PeerId(".to_string()
                }
            }
            NetworkAddress::ChunkAddress(chunk_address) => {
                format!(
                    "NetworkAddress::ChunkAddress({} - ",
                    &chunk_address.to_hex()[0..6]
                )
            }
            NetworkAddress::TransactionAddress(transaction_address) => {
                format!(
                    "NetworkAddress::TransactionAddress({} - ",
                    &transaction_address.to_hex()[0..6]
                )
            }
            NetworkAddress::ScratchpadAddress(scratchpad_address) => {
                format!(
                    "NetworkAddress::ScratchpadAddress({} - ",
                    &scratchpad_address.to_hex()[0..6]
                )
            }
            NetworkAddress::RegisterAddress(register_address) => format!(
                "NetworkAddress::RegisterAddress({} - ",
                &register_address.to_hex()[0..6]
            ),
            NetworkAddress::RecordKey(bytes) => format!(
                "NetworkAddress::RecordKey({} - ",
                &PrettyPrintRecordKey::from(&RecordKey::new(bytes)).no_kbucket_log()[0..6]
            ),
        };
        write!(
            f,
            "{name_str}{:?})",
            PrettyPrintKBucketKey(self.as_kbucket_key()),
        )
    }
}

impl Display for NetworkAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            NetworkAddress::PeerId(id) => {
                write!(f, "NetworkAddress::PeerId({})", hex::encode(id))
            }
            NetworkAddress::ChunkAddress(addr) => {
                write!(f, "NetworkAddress::ChunkAddress({addr:?})")
            }
            NetworkAddress::TransactionAddress(addr) => {
                write!(f, "NetworkAddress::TransactionAddress({addr:?})")
            }
            NetworkAddress::ScratchpadAddress(addr) => {
                write!(f, "NetworkAddress::ScratchpadAddress({addr:?})")
            }
            NetworkAddress::RegisterAddress(addr) => {
                write!(f, "NetworkAddress::RegisterAddress({addr:?})")
            }
            NetworkAddress::RecordKey(key) => {
                write!(f, "NetworkAddress::RecordKey({})", hex::encode(key))
            }
        }
    }
}

/// Pretty print a `kad::KBucketKey` as a hex string.
#[derive(Clone)]
pub struct PrettyPrintKBucketKey(pub Key<Vec<u8>>);

impl std::fmt::Display for PrettyPrintKBucketKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0.hashed_bytes() {
            f.write_fmt(format_args!("{byte:02x}"))?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for PrettyPrintKBucketKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}

/// Provides a hex representation of a `kad::RecordKey`.
///
/// This internally stores the RecordKey as a `Cow` type. Use `PrettyPrintRecordKey::from(&RecordKey)` to create a
/// borrowed version for printing/logging.
/// To use in error messages, to pass to other functions, call `PrettyPrintRecordKey::from(&RecordKey).into_owned()` to
///  obtain a cloned, non-referenced `RecordKey`.
#[derive(Clone, Hash, Eq, PartialEq)]
pub struct PrettyPrintRecordKey<'a> {
    key: Cow<'a, RecordKey>,
}

impl Serialize for PrettyPrintRecordKey<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let record_key_bytes = match &self.key {
            Cow::Borrowed(borrowed_key) => borrowed_key.as_ref(),
            Cow::Owned(owned_key) => owned_key.as_ref(),
        };
        record_key_bytes.serialize(serializer)
    }
}

// Implementing Deserialize for PrettyPrintRecordKey
impl<'de> Deserialize<'de> for PrettyPrintRecordKey<'static> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Deserialize to bytes first
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        // Then use the bytes to create a RecordKey and wrap it in PrettyPrintRecordKey
        Ok(PrettyPrintRecordKey {
            key: Cow::Owned(RecordKey::new(&bytes)),
        })
    }
}
/// This is the only interface to create a PrettyPrintRecordKey.
/// `.into_owned()` must be called explicitly if you want a Owned version to be used for errors/args.
impl<'a> From<&'a RecordKey> for PrettyPrintRecordKey<'a> {
    fn from(key: &'a RecordKey) -> Self {
        PrettyPrintRecordKey {
            key: Cow::Borrowed(key),
        }
    }
}

impl PrettyPrintRecordKey<'_> {
    /// Creates a owned version that can be then used to pass as error values.
    /// Do not call this if you just want to print/log `PrettyPrintRecordKey`
    pub fn into_owned(self) -> PrettyPrintRecordKey<'static> {
        let cloned_key = match self.key {
            Cow::Borrowed(key) => Cow::Owned(key.clone()),
            Cow::Owned(key) => Cow::Owned(key),
        };

        PrettyPrintRecordKey { key: cloned_key }
    }

    pub fn no_kbucket_log(self) -> String {
        let mut content = String::from("");
        let record_key_bytes = match &self.key {
            Cow::Borrowed(borrowed_key) => borrowed_key.as_ref(),
            Cow::Owned(owned_key) => owned_key.as_ref(),
        };
        for byte in record_key_bytes {
            let _ = content.write_fmt(format_args!("{byte:02x}"));
        }
        content
    }
}

impl std::fmt::Display for PrettyPrintRecordKey<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let record_key_bytes = match &self.key {
            Cow::Borrowed(borrowed_key) => borrowed_key.as_ref(),
            Cow::Owned(owned_key) => owned_key.as_ref(),
        };
        // print the first 6 chars
        for byte in record_key_bytes.iter().take(3) {
            f.write_fmt(format_args!("{byte:02x}"))?;
        }

        write!(
            f,
            "({:?})",
            PrettyPrintKBucketKey(NetworkAddress::from_record_key(&self.key).as_kbucket_key())
        )
    }
}

impl std::fmt::Debug for PrettyPrintRecordKey<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // same as display
        write!(f, "{self}")
    }
}

#[cfg(test)]
mod tests {
    use crate::storage::LinkedListAddress;
    use crate::NetworkAddress;
    use bls::rand::thread_rng;

    #[test]
    fn verify_transaction_addr_is_actionable() {
        let xorname = xor_name::XorName::random(&mut thread_rng());
        let transaction_addr = LinkedListAddress::new(xorname);
        let net_addr = NetworkAddress::from_transaction_address(transaction_addr);

        let transaction_addr_hex = &transaction_addr.to_hex()[0..6]; // we only log the first 6 chars
        let net_addr_fmt = format!("{net_addr}");

        assert!(net_addr_fmt.contains(transaction_addr_hex));
    }
}
