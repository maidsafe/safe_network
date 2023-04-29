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

use libp2p::{
    kad::{kbucket::Distance, KBucketKey as Key},
    PeerId,
};
use xor_name::XorName;

/// This is the key in the network by which proximity/distance
/// to other items (wether nodes or data chunks) are calculated.
///
/// This is the mapping from the XOR name used
/// by for example self encryption, or the libp2p PeerId,
/// to the key used in the Kademlia DHT.
/// All our xorname calculations shall be replaced with the KBucketKey calculations,
/// for getting proximity/distance to other items (wether nodes or data chunks).
pub enum NetworkKey {
    PeerId(Vec<u8>),
    XorName(Vec<u8>),
}

impl NetworkKey {
    /// Returns a `NetworkKey` representation of the `XorName` by encapsulating its bytes.
    pub fn from_name(name: XorName) -> Self {
        NetworkKey::XorName(name.0.to_vec())
    }

    /// Returns a `NetworkKey` representation of the `PeerId` by encapsulating its bytes.
    pub fn from_peer_id(peer_id: PeerId) -> Self {
        NetworkKey::PeerId(peer_id.to_bytes())
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        match self {
            NetworkKey::PeerId(bytes) => bytes.to_vec(),
            NetworkKey::XorName(bytes) => bytes.to_vec(),
        }
    }

    pub fn as_kbucket_key(&self) -> Key<Vec<u8>> {
        Key::new(self.as_bytes())
    }

    /// Computes the distance of the keys according to the XOR metric.
    pub fn distance<U>(&self, other: &NetworkKey) -> Distance {
        self.as_kbucket_key().distance(&other.as_kbucket_key())
    }

    // NB: Leaving this here as to demonstrate what we can do with this.
    // /// Returns the uniquely determined key with the given distance to `self`.
    // ///
    // /// This implements the following equivalence:
    // ///
    // /// `self xor other = distance <==> other = self xor distance`
    // pub fn for_distance(&self, d: Distance) -> libp2p::kad::kbucket::KeyBytes {
    //     self.as_kbucket_key().for_distance(d)
    // }
}
