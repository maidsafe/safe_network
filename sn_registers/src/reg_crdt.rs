// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{error::Result, Entry, EntryHash, Error, Metadata, RegisterAddress, RegisterOp, User};

use crdts::{merkle_reg::MerkleReg, CmRDT, CvRDT};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fmt::{self, Debug, Display, Formatter},
    hash::Hash,
};

/// Register data type as a CRDT with Access Control
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd)]
pub(crate) struct RegisterCrdt {
    /// Address on the network of this piece of data
    /// Network address. Omitted when serialising and
    /// calculated from the `metadata` when deserialising.
    address: RegisterAddress,
    /// Metadata provided by the creator of this Register, which becomes immutable,
    /// and it defines this Register's address on the network, i.e. this Register is
    /// stored by the network at: XorName(hash(medatada)).
    metadata: Metadata,
    /// CRDT to store the actual data, i.e. the items of the Register.
    data: MerkleReg<Entry>,
}

impl Display for RegisterCrdt {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "(<<{:?}>>", self.metadata)?;
        for entry in self.data.read().values() {
            write!(f, ", <{entry:?}>")?;
        }
        write!(f, ")")
    }
}

impl RegisterCrdt {
    /// Constructs a new '`RegisterCrdt`' with address derived from the given metadata.
    pub(crate) fn new(metadata: Metadata) -> Self {
        Self {
            address: RegisterAddress::new(metadata.xorname()),
            metadata,
            data: MerkleReg::new(),
        }
    }

    /// Returns the address.
    pub(crate) fn address(&self) -> &RegisterAddress {
        &self.address
    }

    /// Merge another register into this one.
    pub(crate) fn merge(&mut self, other: Self) -> Result<()> {
        // Check the targetting address is correct
        if self.address != other.address {
            return Err(Error::RegisterAddrMismatch {
                dst_addr: self.address,
                reg_addr: other.address,
            });
        }

        self.data.merge(other.data);

        Ok(())
    }

    /// Returns total number of items in the register.
    pub(crate) fn size(&self) -> u64 {
        (self.data.num_nodes() + self.data.num_orphans()) as u64
    }

    /// Write a new entry to the `RegisterCrdt`, returning the hash
    /// of the entry and the CRDT operation without a signature
    pub(crate) fn write(
        &mut self,
        entry: Entry,
        children: BTreeSet<EntryHash>,
        source: User,
    ) -> Result<(EntryHash, RegisterOp)> {
        let address = *self.address();

        let children_array: BTreeSet<[u8; 32]> = children.iter().map(|itr| itr.0).collect();
        let crdt_op = self.data.write(entry, children_array);
        self.data.apply(crdt_op.clone());
        let hash = crdt_op.hash();

        // We return the operation as it may need to be broadcasted to other replicas
        let op = RegisterOp::new(address, crdt_op, source, None);

        Ok((EntryHash(hash), op))
    }

    /// Apply a remote data CRDT operation to this replica of the `RegisterCrdt`.
    pub(crate) fn apply_op(&mut self, op: RegisterOp) -> Result<()> {
        // Let's first check the op is validly signed.
        // Note: Perms and valid sig for the op are checked at the upper Register layer.

        // Check the targetting address is correct
        if self.address != op.address {
            return Err(Error::RegisterAddrMismatch {
                dst_addr: op.address,
                reg_addr: self.address,
            });
        }

        // Apply the CRDT operation to the Register
        self.data.apply(op.crdt_op);

        Ok(())
    }

    /// Get the entry corresponding to the provided `hash` if it exists.
    pub(crate) fn get(&self, hash: EntryHash) -> Option<&Entry> {
        self.data.node(hash.0).map(|node| &node.value)
    }

    /// Read current entries (multiple entries occur on concurrent writes).
    pub(crate) fn read(&self) -> BTreeSet<(EntryHash, Entry)> {
        self.data
            .read()
            .hashes_and_nodes()
            .map(|(hash, node)| (EntryHash(hash), node.value.clone()))
            .collect()
    }

    /// Return this Register's metadata
    pub(crate) fn metadata(&self) -> &Metadata {
        &self.metadata
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rand::Rng;

    #[test]
    fn creating_entry_hash() -> Result<()> {
        let mut rng = rand::thread_rng();
        let metadata_1 = Metadata::new(&rng.gen::<[u8; 32]>())?;
        let metadata_2 = Metadata::new(&rng.gen::<[u8; 32]>())?;

        let mut crdt_1 = RegisterCrdt::new(metadata_1);
        let mut crdt_2 = RegisterCrdt::new(metadata_2);
        let mut parents = BTreeSet::new();

        let entry_1 = vec![0x1, 0x1];
        // Different RegisterCrdt shall create same hashes for the same entry from root
        let (entry_hash_1, _) = crdt_1.write(entry_1.clone(), parents.clone(), User::Anyone)?;
        let (entry_hash_2, _) = crdt_2.write(entry_1, parents.clone(), User::Anyone)?;
        assert!(entry_hash_1 == entry_hash_2);

        let entry_2 = vec![0x2, 0x2];
        // RegisterCrdt shall create differnt hashes for different entries from root
        let (entry_hash_1_2, _) = crdt_1.write(entry_2, parents.clone(), User::Anyone)?;
        assert!(entry_hash_1 != entry_hash_1_2);

        let entry_3 = vec![0x3, 0x3];
        // Different RegisterCrdt shall create same hashes for the same entry from same parents
        let _ = parents.insert(entry_hash_1);
        let (entry_hash_1_3, _) = crdt_1.write(entry_3.clone(), parents.clone(), User::Anyone)?;
        let (entry_hash_2_3, _) = crdt_1.write(entry_3, parents, User::Anyone)?;
        assert!(entry_hash_1_3 == entry_hash_2_3);

        Ok(())
    }
}
