// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::address::TransactionAddress;
use bls::SecretKey;
use serde::{Deserialize, Serialize};

// re-exports
pub use bls::{PublicKey, Signature};

/// Content of a transaction, limited to 32 bytes
pub type TransactionContent = [u8; 32];

/// A generic Transaction on the Network
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash, Ord, PartialOrd)]
pub struct Transaction {
    pub owner: PublicKey,
    pub parents: Vec<PublicKey>,
    pub content: TransactionContent,
    pub outputs: Vec<(PublicKey, TransactionContent)>,
    /// signs the above 4 fields with the owners key
    pub signature: Signature,
}

impl Transaction {
    /// Create a new transaction, signing it with the provided secret key.
    pub fn new(
        owner: PublicKey,
        parents: Vec<PublicKey>,
        content: TransactionContent,
        outputs: Vec<(PublicKey, TransactionContent)>,
        signing_key: &SecretKey,
    ) -> Self {
        let signature = signing_key.sign(Self::bytes_to_sign(&owner, &parents, &content, &outputs));
        Self {
            owner,
            parents,
            content,
            outputs,
            signature,
        }
    }

    /// Create a new transaction, with the signature already calculated.
    pub fn new_with_signature(
        owner: PublicKey,
        parents: Vec<PublicKey>,
        content: TransactionContent,
        outputs: Vec<(PublicKey, TransactionContent)>,
        signature: Signature,
    ) -> Self {
        Self {
            owner,
            parents,
            content,
            outputs,
            signature,
        }
    }

    /// Get the bytes that the signature is calculated from.
    pub fn bytes_to_sign(
        owner: &PublicKey,
        parents: &[PublicKey],
        content: &[u8],
        outputs: &[(PublicKey, TransactionContent)],
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&owner.to_bytes());
        bytes.extend_from_slice("parent".as_bytes());
        bytes.extend_from_slice(
            &parents
                .iter()
                .map(|p| p.to_bytes())
                .collect::<Vec<_>>()
                .concat(),
        );
        bytes.extend_from_slice("content".as_bytes());
        bytes.extend_from_slice(content);
        bytes.extend_from_slice("outputs".as_bytes());
        bytes.extend_from_slice(
            &outputs
                .iter()
                .flat_map(|(p, c)| [&p.to_bytes(), c.as_slice()].concat())
                .collect::<Vec<_>>(),
        );
        bytes
    }

    pub fn address(&self) -> TransactionAddress {
        TransactionAddress::from_owner(self.owner)
    }

    /// Get the bytes that the signature is calculated from.
    pub fn bytes_for_signature(&self) -> Vec<u8> {
        Self::bytes_to_sign(&self.owner, &self.parents, &self.content, &self.outputs)
    }

    pub fn verify(&self) -> bool {
        self.owner
            .verify(&self.signature, self.bytes_for_signature())
    }
}
