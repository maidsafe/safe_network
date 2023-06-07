// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

//! The storage payment proofs are generated using a binary [Merkle tree](https://en.wikipedia.org/wiki/Merkle_tree).
//!
//! A Merkle tree, also known as hash tree, is a data structure used for data verification
//! and synchronization. It is a tree data structure where each non-leaf node is a hash of
//! it’s child nodes. All the leaf nodes are at the same depth and are as far left as
//! possible. It maintains data integrity and uses hash functions for this purpose.
//!
//! In SAFE, in order to pay the network for data storage, all files are first self-encrypted
//! obtaining all the chunks the user needs to pay for before uploading them. A binary Merkle
//! tree is created using all these chunks' addresses/Xornames, each leaf in the Merkle tree
//! holds the value obtained from hashing each of the Chunk's Xorname/address.
//!
//! The following tree depicts how two files A and B, with two chunks each, would be used
//! to build its the merkle tree:
//! ```text
//!                                        [ Root ]
//!                                   hash(Node0 + Node1)
//!                                            ^
//!                                            |
//!                      *-------------------------------------------*
//!                      |                                           |
//!                  [ Node0 ]                                   [ Node1 ]
//!             hash(Leaf0 + Leaf1)                         hash(Leaf2 + Leaf3)
//!                      ^                                           ^
//!                      |                                           |
//!          *----------------------*                    *----------------------*
//!          |                      |                    |                      |
//!      [ Leaf0 ]              [ Leaf1 ]            [ Leaf2 ]              [ Leaf3 ]
//!  hash(ChunkA_0.addr)    hash(ChunkA_1.addr)  hash(ChunkB_0.addr)    hash(ChunkB_1.addr)
//!
//!
//!          ^                      ^                    ^                      ^
//!          ^                      ^                    ^                      ^
//!          |                      |                    |                      |
//!     [ ChunkA_0 ]           [ ChunkA_1 ]         [ ChunkB_0 ]           [ ChunkB_1 ]
//!          ^                      ^                    ^                      ^
//!          |                      |                    |                      |
//!          *----------------------*                    *----------------------*
//!                      |                                           |
//!               self-encryption                             self-encryption
//!                      |                                           |
//!                  [ FileA ]                                   [ FileB ]
//!
//! The user links the payment made to the storing nodes by setting the Merkle tree root value
//! as the 'Dbc::reason_hash' value. Thanks to the properties of the Merkle tree, the user can
//! then provide the output DBC and audit trail for each of the Chunks being payed with the same
//! DBC and tree, to the storage nodes upon uploading the Chunks for storing them on the network.
//! ```

mod error;
mod hasher;

use error::{Error, Result};
use hasher::Sha256Hasher;

use sn_protocol::messages::{Hash, MerkleTreeNodesType, PaymentProof};

use merkletree::{
    hash::Algorithm,
    merkle::{next_pow2, MerkleTree},
    proof::Proof,
    store::VecStore,
};
use std::collections::BTreeMap;
use typenum::{UInt, UTerm, B0, B1};
use xor_name::XorName;

// We use a binary Merkle-tree to build payment proofs
type BinaryMerkletreeProofType = Proof<MerkleTreeNodesType, UInt<UInt<UTerm, B1>, B0>>;

/// Map from content address name to its corresponding PaymentProof
pub type PaymentProofsMap = BTreeMap<MerkleTreeNodesType, PaymentProof>;

/// Build a Merkletree to generate the PaymentProofs for each of the content addresses provided
// TODO: provide fix against https://en.wikipedia.org/wiki/Preimage_attack ?
pub fn build_payment_proofs<'a>(
    content_addrs: impl Iterator<Item = &'a XorName>,
) -> Result<(Hash, PaymentProofsMap)> {
    // Let's build the Merkle-tree from list of addresses needed to generate the payment proofs
    let mut addrs: Vec<_> = content_addrs
        .map(|addr| {
            let mut arr = MerkleTreeNodesType::default();
            // we know the length of a XorName is 32 so we won't miss any byte
            arr.copy_from_slice(addr);
            arr
        })
        .collect();

    if addrs.is_empty() {
        return Err(Error::ProofTree(
            "Cannot build payment proofs with an empty list of addresses".to_string(),
        ));
    }

    // Merkletree requires the number of leaves to be a power of 2, and at least 2 leaves.
    let num_of_leaves = usize::max(2, next_pow2(addrs.len()));
    let num_of_addrs = addrs.len();
    for _ in num_of_addrs..num_of_leaves {
        // fill it up with blank value leaves
        addrs.push(MerkleTreeNodesType::default());
    }

    let merkletree = MerkleTree::<MerkleTreeNodesType, Sha256Hasher, VecStore<_>>::new(
        addrs.clone().into_iter(),
    )
    .map_err(|err| Error::ProofTree(err.to_string()))?;

    // The reason hash is set to be the root of the merkle-tree of chunks to pay for
    let reason_hash = merkletree.root().into();

    let mut payment_proofs = BTreeMap::new();
    for (index, addr) in addrs.into_iter().take(num_of_addrs).enumerate() {
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
                audit_trail: proof.lemma().to_vec(),
                path: proof.path().to_vec(),
            },
        );
    }

    Ok((reason_hash, payment_proofs))
}

/// Verify if the payment proof is valid and contains a valid audit trail for the given xorname
pub fn validate_payment_proof(addr_name: XorName, payment: &PaymentProof) -> Result<()> {
    trace!("Verifying payment proof for chunk store {addr_name:?} ...");

    // We build the merkletree leaf value from the received xorname, i.e. hash(xorname).
    // The DBC's reason-hash should match the root of the PaymentProof's audit trail (lemma)
    let mut hasher = Sha256Hasher::default();
    let leaf_to_validate = hasher.leaf(addr_name.0);

    let proof = BinaryMerkletreeProofType::new::<UTerm, UTerm>(
        None,
        payment.audit_trail.clone(),
        payment.path.clone(),
    )
    .map_err(|err| Error::InvalidAuditTrail(err.to_string()))?;

    if leaf_to_validate != proof.item() {
        return Err(Error::AuditTrailItemMismatch(addr_name));
    }

    if !proof
        .validate::<Sha256Hasher>()
        .map_err(|err| Error::AuditTrailSelfValidation(err.to_string()))?
    {
        return Err(Error::AuditTrailSelfValidation(
            "inclusion proof validation failed".to_string(),
        ));
    }

    if payment.reason_hash == proof.root().into() {
        Ok(())
    } else {
        Err(Error::ReasonHashMismatch(payment.reason_hash.to_hex()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eyre::{eyre, Result};
    use tiny_keccak::{Hasher, Sha3};
    use xor_name::XorName;

    // Helper to generate the sha3-256 hash of the provided bytes, with the provided prefix.
    // We use a prefix since that's what the 'merkletree' crate we use adds to each tree node,
    // e.g. prefix '0' to leaves and prefix '1' to root node.
    fn hash(prefix: u8, bytes_l: &[u8], bytes_r: &[u8]) -> MerkleTreeNodesType {
        let mut sha3 = Sha3::v256();
        sha3.update(&[prefix]);
        sha3.update(bytes_l);
        sha3.update(bytes_r);
        let mut hash = MerkleTreeNodesType::default();
        sha3.finalize(&mut hash);
        hash
    }

    #[test]
    fn test_payment_proof_basic() -> Result<()> {
        let name0 = XorName([11; 32]);
        let name1 = XorName([22; 32]);
        let name2 = XorName([33; 32]);

        let leaf0 = hash(0, &name0.0, &[]);
        let leaf1 = hash(0, &name1.0, &[]);
        let leaf2 = hash(0, &name2.0, &[]);
        let leaf3 = hash(0, &MerkleTreeNodesType::default(), &[]);
        let node0 = hash(1, &leaf0, &leaf1);
        let node1 = hash(1, &leaf2, &leaf3);
        let root = hash(1, &node0, &node1);

        let addrs = [name0, name1, name2];

        let (reason_hash, payment_proofs) = build_payment_proofs(addrs.iter())?;

        assert_eq!(payment_proofs.len(), addrs.len());
        assert_eq!(reason_hash, root.into());

        Ok(())
    }

    #[test]
    fn test_payment_proof_non_power_of_2_input() -> Result<()> {
        assert!(
            matches!(build_payment_proofs(vec![].iter()), Err(Error::ProofTree(err)) if err == "Cannot build payment proofs with an empty list of addresses")
        );

        let addrs = [
            [11; 32], [22; 32], [33; 32], [44; 32], [55; 32], [66; 32], [77; 32], [88; 32],
        ]
        .into_iter()
        .map(XorName)
        .collect::<Vec<_>>();

        for i in 0..addrs.len() {
            let (_, payment_proofs) = build_payment_proofs(addrs.iter().take(i))?;
            assert_eq!(payment_proofs.len(), i);
        }

        Ok(())
    }

    #[test]
    fn test_payment_proof_validation() -> Result<()> {
        let name0 = XorName([11; 32]);
        let name1 = XorName([22; 32]);
        let name2 = XorName([33; 32]);

        let addrs = [name0, name1, name2];

        let (_, payment_proofs) = build_payment_proofs(addrs.iter())?;

        assert_eq!(payment_proofs.len(), addrs.len());

        assert!(
            matches!(payment_proofs.get(&name0.0), Some(proof) if validate_payment_proof(name0, proof).is_ok())
        );
        assert!(
            matches!(payment_proofs.get(&name1.0), Some(proof) if validate_payment_proof(name1, proof).is_ok())
        );
        assert!(
            matches!(payment_proofs.get(&name2.0), Some(proof) if validate_payment_proof(name2, proof).is_ok())
        );

        let mut proof = payment_proofs
            .get(&name2.0)
            .cloned()
            .ok_or_else(|| eyre!("Failed to obtain valid payment proof"))?;
        let invalid_name = XorName([99; 32]);
        assert!(matches!(
            validate_payment_proof(invalid_name, &proof),
            Err(Error::AuditTrailItemMismatch(name)) if name == invalid_name
        ));

        let mut corrupted_proof = proof.clone();
        corrupted_proof.audit_trail[1][0] = 0; // corrupt one byte of the audit trail
        assert!(matches!(
            validate_payment_proof(name2, &corrupted_proof),
            Err(Error::AuditTrailSelfValidation(_))
        ));

        let mut corrupted_proof = proof.clone();
        corrupted_proof.audit_trail.push(name0.0); // corrupt the audit trail by adding some random item
        assert!(matches!(
            validate_payment_proof(name2, &corrupted_proof),
            Err(Error::InvalidAuditTrail(_))
        ));

        let invalid_reason_hash: Hash = [66; 32].into();
        proof.reason_hash = invalid_reason_hash; // corrupt the PaymentProof by setting an invalid reason hash value
        assert!(matches!(
            validate_payment_proof(name2, &proof),
            Err(Error::ReasonHashMismatch(hex)) if hex == invalid_reason_hash.to_hex()
        ));

        Ok(())
    }
}
