// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::ScratchpadAddress;
use crate::NetworkAddress;
use bls::{PublicKey, Signature};
use bytes::Bytes;
use serde::{Deserialize, Serialize};

use xor_name::XorName;

/// Scratchpad, an mutable address for encrypted data
#[derive(
    Hash, Eq, PartialEq, PartialOrd, Ord, Clone, custom_debug::Debug, Serialize, Deserialize,
)]
pub struct Scratchpad {
    /// Network address. Omitted when serialising and
    /// calculated from the `encrypted_data` when deserialising.
    pub address: ScratchpadAddress,
    /// Contained data. This should be encrypted
    #[debug(skip)]
    pub encrypted_data: Bytes,
    /// Monotonically increasing counter to track the number of times this has been updated.
    pub counter: u64,
    /// Signature over `Vec<counter>`.extend(Xorname::from_content(encrypted_data).to_vec()) from the owning key.
    pub signature: Signature,
}

impl Scratchpad {
    /// Creates a new instance of `Scratchpad`.
    pub fn new(
        owner: PublicKey,
        encrypted_data: Bytes,
        counter: u64,
        signature: Signature,
    ) -> Self {
        Self {
            address: ScratchpadAddress::new(owner),
            encrypted_data,
            counter,
            signature,
        }
    }

    /// Verifies the signature and content of the scratchpad are valid for the
    /// owner's public key.
    pub fn is_valid(&self) -> bool {
        let mut signing_bytes = self.counter.to_be_bytes().to_vec();
        signing_bytes.extend(self.encrypted_data_hash().to_vec()); // add the count

        self.owner().verify(&self.signature, &signing_bytes)
    }

    /// Returns the encrypted_data.
    pub fn encrypted_data(&self) -> &Bytes {
        &self.encrypted_data
    }

    /// Returns the encrypted_data hash
    pub fn encrypted_data_hash(&self) -> XorName {
        XorName::from_content(&self.encrypted_data)
    }

    /// Returns the owner.
    pub fn owner(&self) -> &PublicKey {
        self.address.owner()
    }

    /// Returns the address.
    pub fn address(&self) -> &ScratchpadAddress {
        &self.address
    }

    /// Returns the NetworkAddress
    pub fn network_address(&self) -> NetworkAddress {
        NetworkAddress::ScratchpadAddress(self.address)
    }

    /// Returns the name.
    pub fn name(&self) -> XorName {
        self.address.xorname()
    }

    /// Returns size of contained encrypted_data.
    pub fn payload_size(&self) -> usize {
        self.encrypted_data.len()
    }
}
