// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use bls::{PublicKey, SecretKey};
use crdts::merkle_reg::Node as MerkleDagEntry;
use serde::{Deserialize, Serialize};

use crate::{error::Result, Entry, Error, RegisterAddress, User};

/// Register mutation operation to apply to Register.
/// CRDT Data operation applicable to other Register replica.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegisterOp {
    /// Address of a Register object on the network.
    pub(crate) address: RegisterAddress,
    /// The data operation to apply.
    pub(crate) crdt_op: MerkleDagEntry<Entry>,
    /// The PublicKey of the entity that generated the operation
    pub(crate) source: User,
    /// The signature of source on the crdt_op, required to apply the op
    pub(crate) signature: Option<bls::Signature>,
}

impl RegisterOp {
    /// Create a new RegisterOp
    pub fn new(
        address: RegisterAddress,
        crdt_op: MerkleDagEntry<Entry>,
        source: User,
        signature: Option<bls::Signature>,
    ) -> Self {
        Self {
            address,
            crdt_op,
            source,
            signature,
        }
    }

    /// address of the register this op is destined for
    pub fn address(&self) -> RegisterAddress {
        self.address
    }

    /// the entity that generated the operation
    pub fn source(&self) -> User {
        self.source
    }

    /// Add signature to register Op
    pub fn add_signature(&mut self, sk: &SecretKey) -> Result<()> {
        let bytes = bincode::serialize(&self.crdt_op).map_err(|_| Error::SerialisationFailed)?;
        let signature = sk.sign(bytes);
        self.source = User::Key(sk.public_key());
        self.signature = Some(signature);
        Ok(())
    }

    /// Check signature of register Op against provided public key
    pub fn verify_signature(&self, pk: &PublicKey) -> Result<()> {
        let bytes = bincode::serialize(&self.crdt_op).map_err(|_| Error::SerialisationFailed)?;
        let sig = self.signature.as_ref().ok_or(Error::MissingSignature)?;
        if !pk.verify(sig, bytes) {
            return Err(Error::InvalidSignature);
        }
        Ok(())
    }
}
