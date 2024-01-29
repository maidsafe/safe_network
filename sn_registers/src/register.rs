// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    error::Result, reg_crdt::RegisterCrdt, Entry, EntryHash, Error, Permissions, RegisterAddress,
    RegisterOp,
};

use bls::{PublicKey, SecretKey, Signature};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use xor_name::XorName;

/// Arbitrary maximum size of a register entry.
const MAX_REG_ENTRY_SIZE: usize = 1024;

/// Maximum number of entries of a register.
const MAX_REG_NUM_ENTRIES: u16 = 1024;

/// A Register on the SAFE Network
#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Serialize, Deserialize, Debug)]
pub struct Register {
    /// CRDT data of the Register
    crdt: RegisterCrdt,
    /// Permissions of the Register
    /// Depending on the permissions, the owner can allow other users to write to the register
    /// Everyone can always read the Register because all data is public
    permissions: Permissions,
}

/// A Signed Register on the SAFE Network
/// This cryptographically secure version of the Register is used to make sure that the data cannot be tampered with
#[derive(Clone, Debug, Serialize, Deserialize, PartialOrd, PartialEq, Eq, Hash)]
pub struct SignedRegister {
    /// the base register we had at creation
    base_register: Register,
    /// signature over the above by the owner
    signature: Signature,
    /// operations to apply on this register,
    /// they contain a signature of the writer
    ops: BTreeSet<RegisterOp>,
}

impl SignedRegister {
    /// Create a new SignedRegister
    pub fn new(base_register: Register, signature: Signature) -> Self {
        Self {
            base_register,
            signature,
            ops: BTreeSet::new(),
        }
    }

    /// Verfies a SignedRegister
    pub fn verify(&self) -> Result<()> {
        let bytes = self.base_register.bytes()?;
        if !self
            .base_register
            .owner()
            .verify(&self.signature, bytes.as_slice())
        {
            return Err(Error::InvalidSignature);
        }

        for op in &self.ops {
            self.base_register.check_register_op(op)?;
        }
        Ok(())
    }

    pub fn verify_with_address(&self, address: RegisterAddress) -> Result<()> {
        if self.base_register.address() != &address {
            return Err(Error::InvalidRegisterAddress {
                requested: Box::new(address),
                got: Box::new(*self.address()),
            });
        }
        self.verify()
    }

    /// Return the Register after applying all the operations
    pub fn register(self) -> Result<Register> {
        let mut register = self.base_register;
        for op in self.ops {
            register.apply_op(op)?;
        }
        Ok(register)
    }

    /// Merge two SignedRegisters
    pub fn merge(&mut self, other: SignedRegister) -> Result<()> {
        if self.base_register != other.base_register {
            return Err(Error::DifferentBaseRegister);
        }
        self.ops.extend(other.ops);
        Ok(())
    }

    /// Merge two SignedRegisters but verify the incoming content
    /// Significantly slower than merge, use when you want to trust but verify the `other`
    pub fn verified_merge(&mut self, other: SignedRegister) -> Result<()> {
        if self.base_register != other.base_register {
            return Err(Error::DifferentBaseRegister);
        }
        other.verify()?;
        self.ops.extend(other.ops);
        Ok(())
    }

    /// Return the address.
    pub fn address(&self) -> &RegisterAddress {
        self.base_register.address()
    }

    /// Return the owner of the data.
    pub fn owner(&self) -> PublicKey {
        self.base_register.owner()
    }

    /// Check and add an Op to the SignedRegister
    pub fn add_op(&mut self, op: RegisterOp) -> Result<()> {
        self.base_register.check_register_op(&op)?;
        self.ops.insert(op);
        Ok(())
    }
}

impl Register {
    /// Create a new Register
    pub fn new(owner: PublicKey, meta: XorName, mut permissions: Permissions) -> Self {
        let address = RegisterAddress { meta, owner };
        permissions.writers.insert(owner);
        Self {
            crdt: RegisterCrdt::new(address),
            permissions,
        }
    }

    /// Sign a Register and return the signature, makes sure the signer is the owner in the process
    pub fn sign(&self, secret_key: &SecretKey) -> Result<Signature> {
        if self.owner() != secret_key.public_key() {
            return Err(Error::InvalidSecretKey);
        }
        let bytes = self.bytes()?;
        let signature = secret_key.sign(bytes);
        Ok(signature)
    }

    /// Returns a bytes version of the Register used for signing
    /// Use this API when you want to sign a Register withtout providing a secret key to the Register API
    pub fn bytes(&self) -> Result<Vec<u8>> {
        rmp_serde::to_vec(self).map_err(|_| Error::SerialisationFailed)
    }

    /// Sign a Register into a SignedRegister
    pub fn into_signed(self, secret_key: &SecretKey) -> Result<SignedRegister> {
        let signature = self.sign(secret_key)?;
        Ok(SignedRegister::new(self, signature))
    }

    #[cfg(test)]
    pub fn new_owned(owner: PublicKey, meta: XorName) -> Self {
        let permissions = Default::default();
        Self::new(owner, meta, permissions)
    }

    /// Return the address.
    pub fn address(&self) -> &RegisterAddress {
        self.crdt.address()
    }

    /// Return the owner of the data.
    pub fn owner(&self) -> PublicKey {
        self.address().owner()
    }

    /// Return the number of items held in the register
    pub fn size(&self) -> u64 {
        self.crdt.size()
    }

    /// Return a value corresponding to the provided 'hash', if present.
    pub fn get(&self, hash: EntryHash) -> Result<&Entry> {
        self.crdt.get(hash).ok_or(Error::NoSuchEntry(hash))
    }

    /// Return a value corresponding to the provided 'hash', if present.
    pub fn get_cloned(&self, hash: EntryHash) -> Result<Entry> {
        self.crdt.get(hash).cloned().ok_or(Error::NoSuchEntry(hash))
    }

    /// Read the last entry, or entries when there are branches, if the register is not empty.
    pub fn read(&self) -> BTreeSet<(EntryHash, Entry)> {
        self.crdt.read()
    }

    /// Return the permission.
    pub fn permissions(&self) -> &Permissions {
        &self.permissions
    }

    /// Write an entry to the Register, returning the generated
    /// CRDT operation so the caller can sign and broadcast it to other replicas,
    /// along with the hash of the entry just written.
    pub fn write(
        &mut self,
        entry: Entry,
        children: &BTreeSet<EntryHash>,
        signer: &SecretKey,
    ) -> Result<(EntryHash, RegisterOp)> {
        self.check_entry_and_reg_sizes(&entry)?;
        let (hash, address, crdt_op) = self.crdt.write(entry, children)?;
        let op = RegisterOp::new(address, crdt_op, signer);
        Ok((hash, op))
    }

    /// Apply a signed data CRDT operation.
    pub fn apply_op(&mut self, op: RegisterOp) -> Result<()> {
        self.check_entry_and_reg_sizes(&op.crdt_op.value)?;
        self.check_register_op(&op)?;
        self.crdt.apply_op(op)
    }

    /// Merge another Register into this one.
    pub fn merge(&mut self, other: Self) {
        self.crdt.merge(other.crdt);
    }

    /// Check if a register op is valid for our current register
    pub fn check_register_op(&self, op: &RegisterOp) -> Result<()> {
        self.check_user_permissions(op.source)?;
        if self.permissions.anyone_can_write() {
            return Ok(()); // anyone can write, so no need to check the signature
        }

        op.verify_signature(&op.source)
    }

    /// Helper to check user write permissions for the given requester's public key.
    ///
    /// Returns:
    /// `Ok(())` if the user can write to this register
    /// `Err::AccessDenied` if the user cannot write to this register
    pub fn check_user_permissions(&self, requester: PublicKey) -> Result<()> {
        if requester == self.owner() || self.permissions.can_write(&requester) {
            Ok(())
        } else {
            Err(Error::AccessDenied(requester))
        }
    }

    // Private helper to check the given Entry's size is within define limit,
    // as well as check the Register hasn't already reached the maximum number of entries.
    fn check_entry_and_reg_sizes(&self, entry: &Entry) -> Result<()> {
        let size = entry.len();
        if size > MAX_REG_ENTRY_SIZE {
            return Err(Error::EntryTooBig {
                size,
                max: MAX_REG_ENTRY_SIZE,
            });
        }

        let reg_size = self.crdt.size();
        if reg_size >= MAX_REG_NUM_ENTRIES.into() {
            return Err(Error::TooManyEntries(reg_size as usize));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EntryHash, Error, Permissions, Register, RegisterAddress, Result, MAX_REG_NUM_ENTRIES,
    };

    use bls::SecretKey;
    use eyre::Context;
    use proptest::prelude::*;
    use rand::{rngs::OsRng, seq::SliceRandom, thread_rng, Rng};
    use std::{collections::BTreeSet, sync::Arc};
    use xor_name::XorName;

    #[test]
    fn register_create() {
        let meta = xor_name::rand::random();
        let (authority_sk, register) = &gen_reg_replicas(None, meta, None, 1)[0];

        let authority = authority_sk.public_key();
        assert_eq!(register.owner(), authority);
        assert_eq!(register.owner(), authority);

        let address = RegisterAddress::new(meta, authority);
        assert_eq!(*register.address(), address);
    }

    #[test]
    fn register_generate_entry_hash() -> eyre::Result<()> {
        let authority_sk = SecretKey::random();
        let authority = authority_sk.public_key();

        let meta: XorName = xor_name::rand::random();

        let mut replica1 = Register::new_owned(authority, meta);
        let mut replica2 = Register::new_owned(authority, meta);

        // Different item from same replica's root shall having different entry_hash
        let item1 = random_register_entry();
        let item2 = random_register_entry();
        let (entry_hash1_1, _) = replica1.write(item1.clone(), &BTreeSet::new(), &authority_sk)?;
        let (entry_hash1_2, _) = replica1.write(item2, &BTreeSet::new(), &authority_sk)?;
        assert!(entry_hash1_1 != entry_hash1_2);

        // Same item from different replica's root shall remain same
        let (entry_hash2_1, _) = replica2.write(item1, &BTreeSet::new(), &authority_sk)?;
        assert_eq!(entry_hash1_1, entry_hash2_1);

        let mut parents = BTreeSet::new();
        // Different item from different replica with same parents shall be different
        let _ = parents.insert(entry_hash1_1);
        let item3 = random_register_entry();
        let item4 = random_register_entry();
        let (entry_hash1_1_3, _) = replica1.write(item3, &parents, &authority_sk)?;
        let (entry_hash2_1_4, _) = replica2.write(item4, &parents, &authority_sk)?;
        assert!(entry_hash1_1_3 != entry_hash2_1_4);

        Ok(())
    }

    #[test]
    fn register_concurrent_write_ops() -> eyre::Result<()> {
        let authority_sk1 = SecretKey::random();
        let authority1 = authority_sk1.public_key();
        let authority_sk2 = SecretKey::random();
        let authority2 = authority_sk2.public_key();

        let meta: XorName = xor_name::rand::random();

        // We'll have 'authority1' as the owner in both replicas and
        // grant permissions for Write to 'authority2' in both replicas too
        let perms = Permissions::new_with([authority1, authority2]);

        // Instantiate the same Register on two replicas
        let mut replica1 = Register::new(authority_sk1.public_key(), meta, perms);
        let mut replica2 = replica1.clone();

        // And let's write an item to replica1 with autority1
        let item1 = random_register_entry();
        let (_, op1) = replica1.write(item1, &BTreeSet::new(), &authority_sk1)?;

        // Let's assert current state on both replicas
        assert_eq!(replica1.size(), 1);
        assert_eq!(replica2.size(), 0);

        // Concurrently write another item with authority2 on replica2
        let item2 = random_register_entry();
        let (_, op2) = replica2.write(item2, &BTreeSet::new(), &authority_sk2)?;

        // Item should be writed on replica2
        assert_eq!(replica2.size(), 1);

        // Write operations are now broadcasted and applied to both replicas
        replica1.apply_op(op2)?;
        replica2.apply_op(op1)?;

        // Let's assert data convergence on both replicas
        verify_data_convergence(&[replica1, replica2], 2)?;

        Ok(())
    }

    #[test]
    fn register_get_by_hash() -> eyre::Result<()> {
        let (sk, register) = &mut create_reg_replicas(1)[0];

        let entry1 = random_register_entry();
        let entry2 = random_register_entry();
        let entry3 = random_register_entry();

        let (entry1_hash, _) = register.write(entry1.clone(), &BTreeSet::new(), sk)?;

        // this creates a fork since entry1 is not set as child of entry2
        let (entry2_hash, _) = register.write(entry2.clone(), &BTreeSet::new(), sk)?;

        // we'll write entry2 but having the entry1 and entry2 as children,
        // i.e. solving the fork created by them
        let children = [entry1_hash, entry2_hash].into_iter().collect();

        let (entry3_hash, _) = register.write(entry3.clone(), &children, sk)?;

        assert_eq!(register.size(), 3);

        let first_entry = register.get(entry1_hash)?;
        assert_eq!(first_entry, &entry1);

        let second_entry = register.get(entry2_hash)?;
        assert_eq!(second_entry, &entry2);

        let third_entry = register.get(entry3_hash)?;
        assert_eq!(third_entry, &entry3);

        let non_existing_hash = EntryHash::default();
        let entry_not_found = register.get(non_existing_hash);
        assert_eq!(entry_not_found, Err(Error::NoSuchEntry(non_existing_hash)));

        Ok(())
    }

    #[test]
    fn register_query_public_perms() -> eyre::Result<()> {
        let meta = xor_name::rand::random();

        // one register will allow write ops to anyone
        let authority_sk1 = SecretKey::random();
        let authority_pk1 = authority_sk1.public_key();
        let owner1 = authority_pk1;
        let perms1 = Permissions::new_anyone_can_write();
        let replica1 = create_reg_replica_with(meta, Some(authority_sk1), Some(perms1));

        // the other register will allow write ops to 'owner1' and 'owner2' only
        let authority_sk2 = SecretKey::random();
        let authority_pk2 = authority_sk2.public_key();
        let owner2 = authority_pk2;
        let perms2 = Permissions::new_with([owner1]);
        let replica2 = create_reg_replica_with(meta, Some(authority_sk2), Some(perms2));

        // dummy owner
        let sk_rand = SecretKey::random();
        let random_user = sk_rand.public_key();
        let sk_rand2 = SecretKey::random();
        let random_user2 = sk_rand2.public_key();

        // check register 1 is public
        assert_eq!(replica1.owner(), authority_pk1);
        assert_eq!(replica1.check_user_permissions(owner1), Ok(()));
        assert_eq!(replica1.check_user_permissions(owner2), Ok(()));
        assert_eq!(replica1.check_user_permissions(random_user), Ok(()));
        assert_eq!(replica1.check_user_permissions(random_user2), Ok(()));

        // check register 2 has only owner1 and owner2 write allowed
        assert_eq!(replica2.owner(), authority_pk2);
        assert_eq!(replica2.check_user_permissions(owner1), Ok(()));
        assert_eq!(replica2.check_user_permissions(owner2), Ok(()));
        assert_eq!(
            replica2.check_user_permissions(random_user),
            Err(Error::AccessDenied(random_user))
        );
        assert_eq!(
            replica2.check_user_permissions(random_user2),
            Err(Error::AccessDenied(random_user2))
        );

        Ok(())
    }

    #[test]
    fn exceeding_max_reg_entries_errors() -> eyre::Result<()> {
        let meta = xor_name::rand::random();

        // one replica will allow write ops to anyone
        let authority_sk1 = SecretKey::random();
        let perms1 = Permissions::new_anyone_can_write();

        let mut replica = create_reg_replica_with(meta, Some(authority_sk1), Some(perms1));

        for _ in 0..MAX_REG_NUM_ENTRIES {
            let (_hash, _op) = replica
                .write(
                    random_register_entry(),
                    &BTreeSet::new(),
                    &SecretKey::random(),
                )
                .context("Failed to write register entry")?;
        }

        let excess_entry = replica.write(
            random_register_entry(),
            &BTreeSet::new(),
            &SecretKey::random(),
        );

        match excess_entry {
            Err(Error::TooManyEntries(size)) => {
                assert_eq!(size, 1024);
                Ok(())
            }
            anything_else => {
                eyre::bail!(
                    "Expected Excess entries error was not found. Instead: {anything_else:?}"
                )
            }
        }
    }

    // Helpers for tests
    fn gen_reg_replicas(
        authority_sk: Option<SecretKey>,
        meta: XorName,
        perms: Option<Permissions>,
        count: usize,
    ) -> Vec<(SecretKey, Register)> {
        let replicas: Vec<(SecretKey, Register)> = (0..count)
            .map(|_| {
                let authority_sk = authority_sk.clone().unwrap_or_else(SecretKey::random);
                let authority = authority_sk.public_key();
                let perms = perms.clone().unwrap_or_default();
                let register = Register::new(authority, meta, perms);
                (authority_sk, register)
            })
            .collect();

        assert_eq!(replicas.len(), count);
        replicas
    }

    fn create_reg_replicas(count: usize) -> Vec<(SecretKey, Register)> {
        let meta = xor_name::rand::random();

        gen_reg_replicas(None, meta, None, count)
    }

    fn create_reg_replica_with(
        meta: XorName,
        authority_sk: Option<SecretKey>,
        perms: Option<Permissions>,
    ) -> Register {
        let replicas = gen_reg_replicas(authority_sk, meta, perms, 1);
        replicas[0].1.clone()
    }

    // verify data convergence on a set of replicas and with the expected length
    fn verify_data_convergence(replicas: &[Register], expected_size: u64) -> Result<()> {
        // verify all replicas have the same and expected size
        for r in replicas {
            assert_eq!(r.size(), expected_size);
        }

        // now verify that the items are the same in all replicas
        let r0 = &replicas[0];
        for r in replicas {
            assert_eq!(r.crdt, r0.crdt);
        }

        Ok(())
    }

    // Generate a vec of Register replicas of some length, with corresponding vec of keypairs for signing, and the overall owner of the register
    fn generate_replicas(
        max_quantity: usize,
    ) -> impl Strategy<Value = Result<(Vec<Register>, Arc<SecretKey>)>> {
        let xorname = xor_name::rand::random();

        let owner_sk = Arc::new(SecretKey::random());
        let owner = owner_sk.public_key();
        let perms = Permissions::new_anyone_can_write();

        (1..max_quantity + 1).prop_map(move |quantity| {
            let mut replicas = Vec::with_capacity(quantity);
            for _ in 0..quantity {
                let replica = Register::new(owner, xorname, perms.clone());

                replicas.push(replica);
            }

            Ok((replicas, owner_sk.clone()))
        })
    }

    // Generate a Register entry
    fn generate_reg_entry() -> impl Strategy<Value = Vec<u8>> {
        "\\PC*".prop_map(|s| s.into_bytes())
    }

    // Generate a vec of Register entries
    fn generate_dataset(max_quantity: usize) -> impl Strategy<Value = Vec<Vec<u8>>> {
        prop::collection::vec(generate_reg_entry(), 1..max_quantity + 1)
    }

    // Generates a vec of Register entries each with a value suggesting
    // the delivery chance of the op that gets created with the entry
    fn generate_dataset_and_probability(
        max_quantity: usize,
    ) -> impl Strategy<Value = Vec<(Vec<u8>, u8)>> {
        prop::collection::vec((generate_reg_entry(), any::<u8>()), 1..max_quantity + 1)
    }

    proptest! {
        #[test]
        fn proptest_reg_doesnt_crash_with_random_data(
            _data in generate_reg_entry()
        ) {
            // Instantiate the same Register on two replicas
            let meta = xor_name::rand::random();
            let owner_sk = SecretKey::random();
            let perms = Default::default();

            let mut replicas = gen_reg_replicas(
                Some(owner_sk.clone()),
                meta,
                Some(perms),
                2);
            let (_, mut replica1) = replicas.remove(0);
            let (_, mut replica2) = replicas.remove(0);

            // Write an item on replicas
            let (_, op) = replica1.write(random_register_entry(), &BTreeSet::new(), &owner_sk)?;
            replica2.apply_op(op)?;

            verify_data_convergence(&[replica1, replica2], 1)?;
        }

        #[test]
        fn proptest_reg_converge_with_many_random_data(
            dataset in generate_dataset(1000)
        ) {
            // Instantiate the same Register on two replicas
            let meta = xor_name::rand::random();
            let owner_sk = SecretKey::random();
            let perms = Default::default();

            // Instantiate the same Register on two replicas
            let mut replicas = gen_reg_replicas(
                Some(owner_sk.clone()),
                meta,
                Some(perms),
                2);
            let (_, mut replica1) = replicas.remove(0);
            let (_, mut replica2) = replicas.remove(0);

            let dataset_length = dataset.len() as u64;

            // insert our data at replicas
            let mut children = BTreeSet::new();
            for _data in dataset {
                // Write an item on replica1
                let (hash, op) = replica1.write(random_register_entry(), &children, &owner_sk)?;
                // now apply that op to replica 2
                replica2.apply_op(op)?;
                children = vec![hash].into_iter().collect();
            }

            verify_data_convergence(&[replica1, replica2], dataset_length)?;
        }

        #[test]
        fn proptest_reg_converge_with_many_random_data_random_entry_children(
            dataset in generate_dataset(1000)
        ) {
            // Instantiate the same Register on two replicas
            let meta = xor_name::rand::random();
            let owner_sk = SecretKey::random();
            let perms = Default::default();

            // Instantiate the same Register on two replicas
            let mut replicas = gen_reg_replicas(
                Some(owner_sk.clone()),
                meta,
                Some(perms),
                2);
            let (_, mut replica1) = replicas.remove(0);
            let (_, mut replica2) = replicas.remove(0);

            let dataset_length = dataset.len() as u64;

            // insert our data at replicas
            let mut list_of_hashes = Vec::new();
            let mut rng = thread_rng();
            for _data in dataset {
                // choose a random set of children
                let num_of_children: usize = rng.gen();
                let children = list_of_hashes.choose_multiple(&mut OsRng, num_of_children).cloned().collect();

                // Write an item on replica1 using the randomly generated set of children
                let (hash, op) = replica1.write(random_register_entry(), &children, &owner_sk)?;

                // now apply that op to replica 2
                replica2.apply_op(op)?;
                list_of_hashes.push(hash);
            }

            verify_data_convergence(&[replica1, replica2], dataset_length)?;
        }

        #[test]
        fn proptest_reg_converge_with_many_random_data_across_arbitrary_number_of_replicas(
            dataset in generate_dataset(500),
            res in generate_replicas(50)
        ) {
            let (mut replicas, owner_sk) = res?;
            let dataset_length = dataset.len() as u64;

            // insert our data at replicas
            let mut children = BTreeSet::new();
            for _data in dataset {
                // first generate an op from one replica...
                let (hash, op)= replicas[0].write(random_register_entry(), &children, &owner_sk)?;

                // then apply this to all replicas
                for replica in &mut replicas {
                    replica.apply_op(op.clone())?;
                }
                children = vec![hash].into_iter().collect();
            }

            verify_data_convergence(&replicas, dataset_length)?;

        }

        #[test]
        fn proptest_converge_with_shuffled_op_set_across_arbitrary_number_of_replicas(
            dataset in generate_dataset(100),
            res in generate_replicas(500)
        ) {
            let (mut replicas, owner_sk) = res?;
            let dataset_length = dataset.len() as u64;

            // generate an ops set from one replica
            let mut ops = vec![];

            let mut children = BTreeSet::new();
            for _data in dataset {
                let (hash, op) = replicas[0].write(random_register_entry(), &children, &owner_sk)?;
                ops.push(op);
                children = vec![hash].into_iter().collect();
            }

            // now we randomly shuffle ops and apply at each replica
            for replica in &mut replicas {
                let mut ops = ops.clone();
                ops.shuffle(&mut OsRng);

                for op in ops {
                    replica.apply_op(op)?;
                }
            }

            verify_data_convergence(&replicas, dataset_length)?;
        }

        #[test]
        fn proptest_converge_with_shuffled_ops_from_many_replicas_across_arbitrary_number_of_replicas(
            dataset in generate_dataset(1000),
            res in generate_replicas(7)
        ) {
            let (mut replicas, owner_sk) = res?;
            let dataset_length = dataset.len() as u64;

            // generate an ops set using random replica for each data
            let mut ops = vec![];
            let mut children = BTreeSet::new();
            for _data in dataset {
                if let Some(replica) = replicas.choose_mut(&mut OsRng)
                {
                    let (hash, op) = replica.write(random_register_entry(), &children, &owner_sk)?;
                    ops.push(op);
                    children = vec![hash].into_iter().collect();
                }
            }

            let opslen = ops.len() as u64;
            prop_assert_eq!(dataset_length, opslen);

            // now we randomly shuffle ops and apply at each replica
            for replica in &mut replicas {
                let mut ops = ops.clone();
                ops.shuffle(&mut OsRng);

                for op in ops {
                    replica.apply_op(op)?;
                }
            }

            verify_data_convergence(&replicas, dataset_length)?;
        }

        #[test]
        fn proptest_dropped_data_can_be_reapplied_and_we_converge(
            dataset in generate_dataset_and_probability(1000),
        ) {
            // Instantiate the same Register on two replicas
            let meta = xor_name::rand::random();
            let owner_sk = SecretKey::random();
            let perms = Default::default();

            // Instantiate the same Register on two replicas
            let mut replicas = gen_reg_replicas(
                Some(owner_sk.clone()),
                meta,
                Some(perms),
                2);
            let (_, mut replica1) = replicas.remove(0);
            let (_, mut replica2) = replicas.remove(0);

            let dataset_length = dataset.len() as u64;

            let mut ops = vec![];
            let mut children = BTreeSet::new();
            for (_data, delivery_chance) in dataset {
                let (hash, op)= replica1.write(random_register_entry(), &children, &owner_sk)?;

                ops.push((op, delivery_chance));
                children = vec![hash].into_iter().collect();
            }

            for (op, delivery_chance) in ops.clone() {
                if delivery_chance < u8::MAX / 3 {
                    replica2.apply_op(op)?;
                }
            }

            // here we statistically should have dropped some messages
            if dataset_length > 50 {
                assert_ne!(replica2.size(), replica1.size());
            }

            // reapply all ops
            for (op, _) in ops {
                replica2.apply_op(op)?;
            }

            // now we converge
            verify_data_convergence(&[replica1, replica2], dataset_length)?;
        }

        #[test]
        fn proptest_converge_with_shuffled_ops_from_many_while_dropping_some_at_random(
            dataset in generate_dataset_and_probability(1000),
            res in generate_replicas(7),
        ) {
            let (mut replicas, owner_sk) = res?;
            let dataset_length = dataset.len() as u64;

            // generate an ops set using random replica for each data
            let mut ops = vec![];
            let mut children = BTreeSet::new();
            for (_data, delivery_chance) in dataset {
                // a random index within the replicas range
                let index: usize = OsRng.gen_range(0..replicas.len());
                let replica = &mut replicas[index];

                let (hash, op)=replica.write(random_register_entry(), &children, &owner_sk)?;
                ops.push((op, delivery_chance));
                children = vec![hash].into_iter().collect();
            }

            let opslen = ops.len() as u64;
            prop_assert_eq!(dataset_length, opslen);

            // now we randomly shuffle ops and apply at each replica
            for replica in &mut replicas {
                let mut ops = ops.clone();
                ops.shuffle(&mut OsRng);

                for (op, delivery_chance) in ops.clone() {
                    if delivery_chance > u8::MAX / 3 {
                        replica.apply_op(op)?;
                    }
                }

                // reapply all ops, simulating lazy messaging filling in the gaps
                for (op, _) in ops {
                    replica.apply_op(op)?;
                }
            }

            verify_data_convergence(&replicas, dataset_length)?;
        }

        #[test]
        fn proptest_converge_with_shuffled_ops_including_bad_ops_which_error_and_are_not_applied(
            dataset in generate_dataset(10),
            bogus_dataset in generate_dataset(10), // should be same number as dataset
            gen_replicas_result in generate_replicas(10),

        ) {
            let (mut replicas, owner_sk) = gen_replicas_result?;
            let dataset_length = dataset.len();
            let bogus_dataset_length = bogus_dataset.len();
            let number_replicas = replicas.len();

            // generate the real ops set using random replica for each data
            let mut ops = vec![];
            let mut children = BTreeSet::new();
            for _data in dataset {
                if let Some(replica) = replicas.choose_mut(&mut OsRng)
                {
                    let (hash, op)=replica.write(random_register_entry(), &children, &owner_sk)?;
                    ops.push(op);
                    children = vec![hash].into_iter().collect();
                }
            }

            // set up a replica that has nothing to do with the rest, random xor... different owner...
            let xorname = xor_name::rand::random();
            let random_owner_sk = SecretKey::random();
            let mut bogus_replica = Register::new_owned(random_owner_sk.public_key(), xorname);

            // add bogus ops from bogus replica + bogus data
            let mut children = BTreeSet::new();
            for _data in bogus_dataset {
                let (hash, bogus_op) = bogus_replica.write(random_register_entry(), &children, &random_owner_sk)?;
                bogus_replica.apply_op(bogus_op.clone())?;
                ops.push(bogus_op);
                children = vec![hash].into_iter().collect();
            }

            let opslen = ops.len();
            prop_assert_eq!(dataset_length + bogus_dataset_length, opslen);

            let mut err_count = vec![];
            // now we randomly shuffle ops and apply at each replica
            for replica in &mut replicas {
                let mut ops = ops.clone();
                ops.shuffle(&mut OsRng);

                for op in ops {
                    match replica.apply_op(op) {
                        Ok(_) => {},
                        // record all errors to check this matches bogus data
                        Err(error) => {err_count.push(error)},
                    }
                }
            }

            // check we get an error per bogus datum per replica
            assert_eq!(err_count.len(), bogus_dataset_length * number_replicas);

            verify_data_convergence(&replicas, dataset_length as u64)?;
        }
    }

    fn random_register_entry() -> Vec<u8> {
        let random_bytes = thread_rng().gen::<[u8; 32]>();
        random_bytes.to_vec()
    }
}
