// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{error::Result, Entry, Error, RegisterAddress, User};

use bls::PublicKey;
use crdts::merkle_reg::Node as MerkleDagEntry;
use serde::{Deserialize, Serialize};

/// Register mutation operation to apply to Register.
/// CRDT Data operation applicable to other Register replica.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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

impl std::hash::Hash for RegisterOp {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.address.hash(state);
        self.crdt_op.hash().hash(state);
        self.source.hash(state);
        self.signature.hash(state);
    }
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

    /// Add signature to register Op using provided secret key
    pub fn sign_with(&mut self, sk: &bls::SecretKey) -> Result<()> {
        let bytes = self.bytes_for_signing()?;
        let signature = sk.sign(bytes);
        self.source = User::Key(sk.public_key());
        self.signature = Some(signature);
        Ok(())
    }

    /// Manually add signature to register Op
    pub fn add_signature(
        &mut self,
        public_key: PublicKey,
        signature: bls::Signature,
    ) -> Result<()> {
        self.source = User::Key(public_key);
        self.signature = Some(signature);
        Ok(())
    }

    /// Returns a bytes version of the RegisterOp used for signing
    /// Use this API when you want to sign a RegisterOp withtout providing a secret key to the RegisterOp API
    pub fn bytes_for_signing(&self) -> Result<Vec<u8>> {
        bincode::serialize(&self.crdt_op).map_err(|_| Error::SerialisationFailed)
    }

    /// Check signature of register Op against provided public key
    pub fn verify_signature(&self, pk: &PublicKey) -> Result<()> {
        let bytes = self.bytes_for_signing()?;
        let sig = self.signature.as_ref().ok_or(Error::MissingSignature)?;
        if !pk.verify(sig, bytes) {
            return Err(Error::InvalidSignature);
        }
        Ok(())
    }
}
