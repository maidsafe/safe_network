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
//! The user links the payment made to the storing nodes by saving the Merkle tree root value
//! in the DBC's fee output info. Thanks to the properties of the Merkle tree, the user can
//! then provide the TX, and audit trail for each of the Chunks being payed with the same
//! transaction and tree, to the storage nodes upon uploading the Chunks for storing them on the network.
//! ```

pub(crate) mod error;
mod hasher;

use error::{Error, Result};
use hasher::Sha256Hasher;

use sn_protocol::messages::{Hash, MerkleTreeNodesType};

use merkletree::{
    hash::Algorithm,
    merkle::{next_pow2, MerkleTree},
    proof::Proof,
    store::VecStore,
};
use std::collections::BTreeMap;
use typenum::{UInt, UTerm, B0, B1};
use xor_name::XorName;

/// Map from content address name to its corresponding audit trail and trail path.
pub type PaymentProofsTrailInfoMap = BTreeMap<XorName, (Vec<MerkleTreeNodesType>, Vec<usize>)>;

// We use a binary Merkle-tree to build payment proofs
type BinaryMerkletreeProofType = Proof<MerkleTreeNodesType, UInt<UInt<UTerm, B1>, B0>>;

/// Build a Merkletree to generate the audit trail and path for each of the content addresses provided.
/// The order of the addresses will be kept thus their corresponding leaves in the built tree will be at the same index.
pub fn build_payment_proofs<'a>(
    content_addrs: impl Iterator<Item = &'a XorName>,
) -> Result<(Hash, PaymentProofsTrailInfoMap)> {
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

    // The root hash is the root of the merkle-tree of chunks to pay for
    let root_hash = merkletree.root().into();

    let mut payment_proofs = BTreeMap::new();
    for (index, addr) in addrs.into_iter().take(num_of_addrs).enumerate() {
        let proof = merkletree
            .gen_proof(index)
            .map_err(|err| Error::GenAuditTrail {
                index,
                reason: err.to_string(),
            })?;

        payment_proofs.insert(
            XorName(addr),
            (proof.lemma().to_vec(), proof.path().to_vec()),
        );
    }

    Ok((root_hash, payment_proofs))
}

/// Verify if the payment proof is valid and contains a valid audit trail for the given xorname,
/// returning the Merkletree leaf index for the item realised from the provided path.
pub fn validate_payment_proof(
    addr_name: XorName,
    root_hash: &Hash,
    audit_trail: &[MerkleTreeNodesType],
    path: &[usize],
) -> Result<usize> {
    trace!("Verifying payment proof for chunk store {addr_name:?} ...");

    let proof =
        BinaryMerkletreeProofType::new::<UTerm, UTerm>(None, audit_trail.to_vec(), path.to_vec())
            .map_err(|err| Error::InvalidAuditTrail(err.to_string()))?;

    // We build the merkletree leaf value from the received xorname, i.e. hash(xorname).
    let mut hasher = Sha256Hasher::default();
    let leaf_to_validate = hasher.leaf(addr_name.0);

    if leaf_to_validate != proof.item() {
        return Err(Error::AuditTrailItemMismatch(addr_name));
    }

    // The root-hash should match the root of the Merkletree
    if *root_hash != proof.root().into() {
        return Err(Error::RootHashMismatch(*root_hash));
    }

    if !proof
        .validate::<Sha256Hasher>()
        .map_err(|err| Error::AuditTrailSelfValidation(err.to_string()))?
    {
        return Err(Error::AuditTrailSelfValidation(
            "inclusion proof validation failed".to_string(),
        ));
    }

    let leaf_index = leaf_index(path);

    Ok(leaf_index)
}

// Return the leaf index realised from the provided auth trail path
fn leaf_index(path: &[usize]) -> usize {
    let mut leaf_index = 0;
    path.iter().rev().for_each(|p| {
        leaf_index = (leaf_index << 1) + p;
    });
    leaf_index
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

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
        assert!(
            matches!(build_payment_proofs([].iter()), Err(Error::ProofTree(err)) if err == "Cannot build payment proofs with an empty list of addresses")
        );

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

        let (root_hash, payment_proofs) = build_payment_proofs(addrs.iter())?;

        assert_eq!(payment_proofs.len(), addrs.len());
        assert_eq!(root_hash, root.into());

        Ok(())
    }

    #[test]
    fn test_payment_proof_validation() -> Result<()> {
        let name0 = XorName([11; 32]);
        let name1 = XorName([22; 32]);
        let name2 = XorName([33; 32]);

        let addrs = [name0, name1, name2];

        let (root_hash, payment_proofs) = build_payment_proofs(addrs.iter())?;

        assert_eq!(payment_proofs.len(), addrs.len());

        assert!(
            matches!(payment_proofs.get(&name0), Some((audit_trail, path))
                if matches!(validate_payment_proof(name0, &root_hash, audit_trail, path), Ok(0))
            )
        );
        assert!(
            matches!(payment_proofs.get(&name1), Some((audit_trail, path))
                if matches!(validate_payment_proof(name1, &root_hash, audit_trail, path), Ok(1))
            )
        );
        assert!(
            matches!(payment_proofs.get(&name2), Some(( audit_trail, path))
                if matches!(validate_payment_proof(name2, &root_hash, audit_trail, path), Ok(2))
            )
        );

        let (audit_trail, path) = payment_proofs
            .get(&name2)
            .cloned()
            .ok_or_else(|| eyre!("Failed to obtain valid payment proof"))?;
        let invalid_name = XorName([99; 32]);
        assert!(matches!(
            validate_payment_proof(invalid_name, &root_hash, &audit_trail, &path),
            Err(Error::AuditTrailItemMismatch(name)) if name == invalid_name
        ));

        let mut corrupted_audit_trail = audit_trail.clone();
        corrupted_audit_trail[1][0] = 0; // corrupt one byte of the audit trail
        assert!(matches!(
            validate_payment_proof(name2, &root_hash, &corrupted_audit_trail, &path),
            Err(Error::AuditTrailSelfValidation(_))
        ));

        let mut corrupted_audit_trail = audit_trail.clone();
        corrupted_audit_trail.push(name0.0); // corrupt the audit trail by adding some random item
        assert!(matches!(
            validate_payment_proof(name2, &root_hash, &corrupted_audit_trail, &path),
            Err(Error::InvalidAuditTrail(_))
        ));

        let invalid_root_hash: Hash = [66; 32].into();
        assert!(matches!(
            validate_payment_proof(name2, &invalid_root_hash, &audit_trail, &path),
            Err(Error::RootHashMismatch(root_hash)) if root_hash == invalid_root_hash
        ));

        Ok(())
    }

    proptest! {
        #[test]
        fn test_payment_proof_non_power_of_2_input(num_of_addrs in 1..1000) {
            let mut rng = rand::thread_rng();
            let random_names = (0..num_of_addrs).map(|_| XorName::random(&mut rng)).collect::<Vec<_>>();

            let (root_hash, payment_proofs) = build_payment_proofs(random_names.iter())?;
            assert_eq!(payment_proofs.len(), num_of_addrs as usize);

            for (index, xorname) in random_names.into_iter().enumerate() {
                let (audit_trail, path) = payment_proofs.get(&xorname).expect("Missing payment proof for addr: {addr:?}");
                assert_eq!(leaf_index(path), index);
                matches!(validate_payment_proof(xorname, &root_hash, audit_trail, path), Ok(leaf_index) if leaf_index == index);
            }
        }
    }
}
