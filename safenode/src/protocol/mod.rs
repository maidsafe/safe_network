// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

/// Errors.
pub mod error;
/// Messages types
pub mod messages;

use self::error::{Error, Result};

use libp2p::{
    kad::{kbucket::Distance, KBucketKey as Key},
    PeerId,
};
use serde::{Deserialize, Serialize};
use xor_name::{XorName, XOR_NAME_LEN};

/// This is the key in the network by which proximity/distance
/// to other items (wether nodes or data chunks) are calculated.
///
/// This is the mapping from the XOR name used
/// by for example self encryption, or the libp2p `PeerId`,
/// to the key used in the Kademlia DHT.
/// All our xorname calculations shall be replaced with the `KBucketKey` calculations,
/// for getting proximity/distance to other items (wether nodes or data).
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Debug)]
pub enum NetworkKey {
    /// The `PeerId` of a node.
    PeerId(Vec<u8>),
    /// The `XorName` of some data.
    XorName(Vec<u8>),
}

impl NetworkKey {
    /// Return a `NetworkKey` representation of the `XorName` by encapsulating its bytes.
    pub fn from_name(name: XorName) -> Self {
        NetworkKey::XorName(name.0.to_vec())
    }

    /// Return a `NetworkKey` representation of the `PeerId` by encapsulating its bytes.
    pub fn from_peer(peer_id: PeerId) -> Self {
        NetworkKey::PeerId(peer_id.to_bytes())
    }

    /// Return the encapsulated bytes of this `NetworkKey`.
    pub fn as_bytes(&self) -> Vec<u8> {
        match self {
            NetworkKey::PeerId(bytes) => bytes.to_vec(),
            NetworkKey::XorName(bytes) => bytes.to_vec(),
        }
    }

    /// Try to convert this `NetworkKey` to an `XorName`.
    pub fn as_name(&self) -> Result<XorName> {
        let bytes = match self {
            NetworkKey::PeerId(bytes) => {
                return Err(Error::InternalProcessing(format!(
                    "Not an xorname: {bytes:?}"
                )))
            }
            NetworkKey::XorName(bytes) => bytes,
        };
        let mut xor = [0u8; XOR_NAME_LEN];
        xor.copy_from_slice(&bytes[..XOR_NAME_LEN]);
        Ok(XorName(xor))
    }

    /// Try to convert this `NetworkKey` to a `PeerId`.
    pub fn as_peer(&self) -> Result<PeerId> {
        let bytes = match self {
            NetworkKey::PeerId(bytes) => bytes.to_vec(),
            NetworkKey::XorName(bytes) => {
                return Err(Error::InternalProcessing(format!(
                    "Not a peer id: {bytes:?}"
                )))
            }
        };
        match PeerId::from_bytes(&bytes) {
            Ok(peer_id) => Ok(peer_id),
            Err(err) => Err(Error::InternalProcessing(format!(
                "Invalid peer id bytes: {err}"
            ))),
        }
    }

    /// Return the `KBucketKey` representation of this `NetworkKey`.
    ///
    /// The `KBucketKey` is used for calculating proximity/distance to other items (wether nodes or data).
    /// Important to note is that it will always SHA256 hash any bytes it receives.
    /// Therefore, the canonical use of distance/proximity calculations in the network
    /// is via the `KBucketKey`, or the convenience methods of `NetworkKey`.
    pub fn as_kbucket_key(&self) -> Key<Vec<u8>> {
        Key::new(self.as_bytes())
    }

    /// Compute the distance of the keys according to the XOR metric.
    pub fn distance(&self, other: &NetworkKey) -> Distance {
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
