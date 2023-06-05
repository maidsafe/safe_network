// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod error;
mod hasher;

use error::{Error, Result};
use hasher::Sha256Hasher;

use sn_protocol::{
    messages::{Hash, PaymentProof},
    NetworkAddress,
};

use merkletree::{
    hash::Algorithm,
    merkle::{next_pow2, MerkleTree},
    proof::Proof,
    store::VecStore,
};
use std::collections::BTreeMap;
use typenum::{UInt, UTerm, B0, B1};
use xor_name::XorName;

// Data type of each of the nodes in the binary Merkle-tree we build payment proofs with
type MerkleTreeNodesType = [u8; 32];

// We use a binary Merkle-tree to build payment proofs
type BinaryMerkletreeProofType = Proof<MerkleTreeNodesType, UInt<UInt<UTerm, B1>, B0>>;

/// Map from content address name to its corresponding PaymentProof
pub type PaymentProofsMap = BTreeMap<[u8; 32], PaymentProof>;

/// Build a Merkletree to generate the PaymentProofs for each of the content addresses provided
pub fn build_payment_proofs<'a>(
    content_addrs: impl Iterator<Item = &'a NetworkAddress>,
) -> Result<(Hash, PaymentProofsMap)> {
    // Let's build the Merkle-tree from list of addresses needed to generate the payment proofs
    let mut addrs: Vec<_> = content_addrs
        .map(|addr| {
            let mut arr = MerkleTreeNodesType::default();
            // TODO: check the length of addr is 32 so we don't (unexpectedly) miss bytes...?
            arr.copy_from_slice(&addr.as_bytes());
            arr
        })
        .collect();

    // Merkletree requires the number of leaves to be a power of 2, and at least 2 leaves.
    let num_of_leaves = usize::max(2, next_pow2(addrs.len()));
    for _ in addrs.len()..num_of_leaves {
        // fill it up with blank value leafs
        addrs.push(MerkleTreeNodesType::default());
    }

    let merkletree = MerkleTree::<MerkleTreeNodesType, Sha256Hasher, VecStore<_>>::new(
        addrs.clone().into_iter(),
    )
    .map_err(|err| Error::ProofTree(err.to_string()))?;

    // The reason hash is set to be the root of the merkle-tree of chunks to pay for
    let reason_hash = merkletree.root().into();

    let mut payment_proofs = BTreeMap::new();
    for (index, addr) in addrs.into_iter().enumerate() {
        let proof = merkletree
            .gen_proof(index)
            .map_err(|err| Error::GenAuditTrail {
                index,
                reason: err.to_string(),
            })?;

        payment_proofs.insert(
            addr,
            PaymentProof {
                reason_hash,
                lemma: proof.lemma().to_vec(),
                path: proof.path().to_vec(),
            },
        );
    }

    Ok((reason_hash, payment_proofs))
}

/// Verify if the payment proof is valid and contains a valid audit trail for the given xorname
pub fn validate_payment_proof(addr_name: XorName, payment: PaymentProof) -> Result<()> {
    trace!("Verifying payment proof for chunk store {addr_name:?} ...");

    // We build the merkletree leaf value from the received xorname, i.e. hash(xorname).
    // The DBC's reason-hash should match the root of the PaymentProof's audit trail (lemma)
    let mut hasher = Sha256Hasher::default();
    let leaf_to_validate = hasher.leaf(addr_name.0);
    trace!(">>=== LEAF from Chunk: {leaf_to_validate:?}");

    let proof = BinaryMerkletreeProofType::new::<UTerm, UTerm>(None, payment.lemma, payment.path)
        .map_err(|err| Error::InvalidAuditTrail(err.to_string()))?;

    trace!(">>=== PROOF RECEIVED for Chunk: {proof:?}");

    if leaf_to_validate != proof.item() {
        trace!(">> LEAF doesn't match");
        return Err(Error::AuditTrailItemMismatch(addr_name));
    }

    let proof_validated = proof
        .validate::<Sha256Hasher>()
        .map_err(|err| Error::AuditTrailSelfValidation(err.to_string()))?;

    trace!(">> LEAF matched!. PROOF validated? {proof_validated}");
    if !proof_validated {
        return Err(Error::AuditTrailSelfValidation(
            "inclusion proof validation failed".to_string(),
        ));
    }

    let root: Hash = proof.root().into();
    let root_matched = root == payment.reason_hash;
    trace!(">> ROOT matched ?: {root:?} ==> {root_matched}");

    if root_matched {
        Ok(())
    } else {
        Err(Error::ReasonHashMismatch(payment.reason_hash.to_hex()))
    }
}
