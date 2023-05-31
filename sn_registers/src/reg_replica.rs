// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sn_protocol::{
    error::Error,
    storage::{
        registers::{Action, Entry, EntryHash, Permissions, Policy, Register, RegisterOp, User},
        RegisterAddress,
    },
};

use super::{reg_crdt::RegisterCrdtImpl, Result};

use self_encryption::MIN_ENCRYPTABLE_BYTES;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, hash::Hash};
use xor_name::XorName;

/// Arbitrary maximum size of a register entry.
const MAX_REG_ENTRY_SIZE: usize = MIN_ENCRYPTABLE_BYTES / 3; // 1024 bytes

/// Maximum number of entries of a register.
const MAX_REG_NUM_ENTRIES: u16 = 1024;

/// Object storing the data of a Register, implementing the functions for CRDT operations
#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Serialize, Deserialize, Debug)]
pub struct RegisterReplica {
    authority: User,
    crdt: RegisterCrdtImpl,
    policy: Policy,
}

impl From<Register> for RegisterReplica {
    fn from(reg: Register) -> Self {
        Self {
            authority: reg.authority,
            crdt: reg.crdt.into(),
            policy: reg.policy,
        }
    }
}

// We allow from_over_into since the `RegisterReplica` is not parrt of this crate or implementation.
#[allow(clippy::from_over_into)]
impl Into<Register> for RegisterReplica {
    fn into(self) -> Register {
        Register {
            authority: self.authority,
            crdt: self.crdt.into(),
            policy: self.policy,
        }
    }
}

impl RegisterReplica {
    ///
    pub fn new(authority: User, name: XorName, tag: u64, policy: Policy) -> Self {
        let address = RegisterAddress { name, tag };
        Self {
            authority,
            crdt: RegisterCrdtImpl::new(address),
            policy,
        }
    }

    #[cfg(test)]
    pub fn new_owned(authority: User, name: XorName, tag: u64) -> Self {
        Self::new(
            authority,
            name,
            tag,
            Policy {
                owner: authority,
                permissions: Default::default(),
            },
        )
    }

    /// Return the address.
    pub fn address(&self) -> &RegisterAddress {
        self.crdt.address()
    }

    /// Return the name.
    pub fn name(&self) -> &XorName {
        self.address().name()
    }

    /// Return the tag.
    pub fn tag(&self) -> u64 {
        self.address().tag()
    }

    /// Return the owner of the data.
    pub fn owner(&self) -> User {
        self.policy.owner
    }

    /// Return the PK which the messages are expected to be signed with by this replica.
    #[cfg(test)]
    pub fn replica_authority(&self) -> User {
        self.authority
    }

    /// Return the number of items held in the register
    pub fn size(&self) -> u64 {
        self.crdt.size()
    }

    /// Return a value corresponding to the provided 'hash', if present.
    pub fn get(&self, hash: EntryHash) -> Result<&Entry> {
        self.crdt.get(hash).ok_or(Error::NoSuchEntry(hash))
    }

    /// Read the last entry, or entries when there are branches, if the register is not empty.
    pub fn read(&self) -> BTreeSet<(EntryHash, Entry)> {
        self.crdt.read()
    }

    /// Return user permissions, if applicable.
    pub fn permissions(&self, user: User) -> Result<Permissions> {
        if user == self.policy.owner {
            // i.e. it won't be possible to circumvent the semantics of `owner`
            // by setting some other permissions for the user.
            // the permissions can still be kept in the state though, so that switching owners gives an immediate permission update as well
            Ok(Permissions::new(true))
        } else {
            self.policy
                .permissions
                .get(&user)
                .copied()
                .ok_or(Error::NoSuchUser(user))
        }
    }

    /// Return the policy.
    pub fn policy(&self) -> &Policy {
        &self.policy
    }

    /// Write an entry to the Register, returning the generated unsigned
    /// CRDT operation so the caller can sign and broadcast it to other replicas,
    /// along with the hash of the entry just written.
    pub fn write(
        &mut self,
        entry: Entry,
        children: BTreeSet<EntryHash>,
    ) -> Result<(EntryHash, RegisterOp<Entry>)> {
        self.check_entry_and_reg_sizes(&entry)?;
        self.crdt.write(entry, children, self.authority)
    }

    /// Apply a signed data CRDT operation.
    pub fn apply_op(&mut self, op: RegisterOp<Entry>) -> Result<()> {
        self.check_entry_and_reg_sizes(&op.crdt_op.value)?;
        self.crdt.apply_op(op)
    }

    /// Merge another Register into this one.
    pub fn merge(&mut self, other: Self) {
        self.crdt.merge(other.crdt);
    }

    /// Helper to check permissions for given `action`
    /// for the given requester's public key.
    ///
    /// Returns:
    /// `Ok(())` if the permissions are valid,
    /// `Err::AccessDenied` if the action is not allowed.
    pub fn check_permissions(&self, action: Action, requester: Option<User>) -> Result<()> {
        let requester = requester.unwrap_or(self.authority);
        // First checks if the requester is the owner.
        if action == Action::Read || requester == self.policy.owner {
            Ok(())
        } else {
            match self
                .is_action_allowed_by_user(&requester, action)
                .or_else(|| self.is_action_allowed_by_user(&User::Anyone, action))
            {
                Some(true) => Ok(()),
                Some(false) => Err(Error::AccessDenied(requester)),
                None => Err(Error::AccessDenied(requester)),
            }
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

    // Returns `Some(true)` if `action` is allowed for the provided user and `Some(false)` if it's
    // not permitted. `None` means that default permissions should be applied.
    fn is_action_allowed_by_user(&self, user: &User, action: Action) -> Option<bool> {
        self.policy
            .permissions
            .get(user)
            .and_then(|perms| perms.is_allowed(action))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Entry, EntryHash, Error, Permissions, Policy, RegisterAddress, RegisterOp, RegisterReplica,
        Result, User, MAX_REG_NUM_ENTRIES,
    };

    use bls::SecretKey;
    use eyre::Context;
    use proptest::prelude::*;
    use rand::{rngs::OsRng, seq::SliceRandom, thread_rng, Rng};
    use std::{
        collections::{BTreeMap, BTreeSet},
        sync::Arc,
    };
    use xor_name::XorName;

    #[test]
    fn register_create() {
        let name = xor_name::rand::random();
        let tag = 43_000;
        let (authority_sk, register) = &gen_reg_replicas(None, name, tag, None, 1)[0];

        assert_eq!(*register.name(), name);
        assert_eq!(register.tag(), tag);

        let authority = User::Key(authority_sk.public_key());
        assert_eq!(register.owner(), authority);
        assert_eq!(register.replica_authority(), authority);

        let address = RegisterAddress::new(name, tag);
        assert_eq!(*register.address(), address);
    }

    #[test]
    fn register_generate_entry_hash() -> Result<()> {
        let authority_sk = SecretKey::random();
        let authority = User::Key(authority_sk.public_key());

        let name: XorName = xor_name::rand::random();
        let tag = 43_000u64;

        let mut replica1 = RegisterReplica::new_owned(authority, name, tag);
        let mut replica2 = RegisterReplica::new_owned(authority, name, tag);

        // Different item from same replica's root shall having different entry_hash
        let item1 = random_register_entry();
        let item2 = random_register_entry();
        let (entry_hash1_1, _) = replica1.write(item1.clone(), BTreeSet::new())?;
        let (entry_hash1_2, _) = replica1.write(item2, BTreeSet::new())?;
        assert!(entry_hash1_1 != entry_hash1_2);

        // Same item from different replica's root shall remain same
        let (entry_hash2_1, _) = replica2.write(item1, BTreeSet::new())?;
        assert_eq!(entry_hash1_1, entry_hash2_1);

        let mut parents = BTreeSet::new();
        // Different item from different replica with same parents shall be different
        let _ = parents.insert(entry_hash1_1);
        let item3 = random_register_entry();
        let item4 = random_register_entry();
        let (entry_hash1_1_3, _) = replica1.write(item3, parents.clone())?;
        let (entry_hash2_1_4, _) = replica2.write(item4, parents.clone())?;
        assert!(entry_hash1_1_3 != entry_hash2_1_4);

        Ok(())
    }

    #[test]
    fn register_concurrent_write_ops() -> Result<()> {
        let authority_sk1 = SecretKey::random();
        let authority1 = User::Key(authority_sk1.public_key());
        let authority_sk2 = SecretKey::random();
        let authority2 = User::Key(authority_sk2.public_key());

        let name: XorName = xor_name::rand::random();
        let tag = 43_000u64;

        // We'll have 'authority1' as the owner in both replicas and
        // grant permissions for Write to 'authority2' in both replicas too
        let mut perms = BTreeMap::default();
        let user_perms = Permissions::new(true);
        let _prev = perms.insert(authority2, user_perms);

        // Instantiate the same Register on two replicas with the two diff authorities
        let mut replica1 = RegisterReplica::new(
            authority1,
            name,
            tag,
            Policy {
                owner: authority1,
                permissions: perms.clone(),
            },
        );
        let mut replica2 = RegisterReplica::new(
            authority2,
            name,
            tag,
            Policy {
                owner: authority1,
                permissions: perms,
            },
        );

        // And let's write an item to replica1 with autority1
        let item1 = random_register_entry();
        let (_, op1) = replica1.write(item1, BTreeSet::new())?;
        let signed_write_op1 = sign_register_op(op1, &authority_sk1)?;

        // Let's assert current state on both replicas
        assert_eq!(replica1.size(), 1);
        assert_eq!(replica2.size(), 0);

        // Concurrently write another item with authority2 on replica2
        let item2 = random_register_entry();
        let (_, op2) = replica2.write(item2, BTreeSet::new())?;
        let signed_write_op2 = sign_register_op(op2, &authority_sk2)?;

        // Item should be writeed on replica2
        assert_eq!(replica2.size(), 1);

        // Write operations are now broadcasted and applied to both replicas
        replica1.apply_op(signed_write_op2)?;
        replica2.apply_op(signed_write_op1)?;

        // Let's assert data convergence on both replicas
        verify_data_convergence(vec![replica1, replica2], 2)?;

        Ok(())
    }

    #[test]
    fn register_get_by_hash() -> eyre::Result<()> {
        let (_, register) = &mut create_reg_replicas(1)[0];

        let entry1 = random_register_entry();
        let entry2 = random_register_entry();
        let entry3 = random_register_entry();

        let (entry1_hash, _) = register.write(entry1.clone(), BTreeSet::new())?;

        // this creates a fork since entry1 is not set as child of entry2
        let (entry2_hash, _) = register.write(entry2.clone(), BTreeSet::new())?;

        // we'll write entry2 but having the entry1 and entry2 as children,
        // i.e. solving the fork created by them
        let children = vec![entry1_hash, entry2_hash].into_iter().collect();

        let (entry3_hash, _) = register.write(entry3.clone(), children)?;

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
    fn register_query_public_policy() -> eyre::Result<()> {
        let name = xor_name::rand::random();
        let tag = 43_666;

        // one replica will allow write ops to anyone
        let authority_sk1 = SecretKey::random();
        let owner1 = User::Key(authority_sk1.public_key());
        let mut perms1 = BTreeMap::default();
        let _prev = perms1.insert(User::Anyone, Permissions::new(true));
        let replica1 = create_reg_replica_with(
            name,
            tag,
            Some(authority_sk1),
            Some(Policy {
                owner: owner1,
                permissions: perms1,
            }),
        );

        // the other replica will allow write ops to 'owner1' and 'authority2' only
        let authority_sk2 = SecretKey::random();
        let authority2 = User::Key(authority_sk2.public_key());
        let mut perms2 = BTreeMap::default();
        let _prev = perms2.insert(owner1, Permissions::new(true));
        let replica2 = create_reg_replica_with(
            name,
            tag,
            Some(authority_sk2),
            Some(Policy {
                owner: authority2,
                permissions: perms2,
            }),
        );

        assert_eq!(replica1.owner(), owner1);
        assert_eq!(replica1.replica_authority(), owner1);
        assert_eq!(
            replica1.permissions(User::Anyone),
            Ok(Permissions::new(true)),
        );
        assert_eq!(replica1.permissions(User::Anyone)?, Permissions::new(true),);

        assert_eq!(replica2.owner(), authority2);
        assert_eq!(replica2.replica_authority(), authority2);
        assert_eq!(replica2.permissions(owner1), Ok(Permissions::new(true)),);
        assert_eq!(replica2.permissions(owner1)?, Permissions::new(true),);

        let random_sk = SecretKey::random();
        let random_user = User::Key(random_sk.public_key());
        assert_eq!(
            replica2.permissions(random_user),
            Err(Error::NoSuchUser(random_user))
        );

        Ok(())
    }

    #[test]
    fn exceeding_max_reg_entries_errors() -> eyre::Result<()> {
        let name = xor_name::rand::random();
        let tag = 43_666;

        // one replica will allow write ops to anyone
        let authority_sk1 = SecretKey::random();
        let owner1 = User::Key(authority_sk1.public_key());
        let mut perms1 = BTreeMap::default();
        let _prev = perms1.insert(User::Anyone, Permissions::new(true));
        let mut replica = create_reg_replica_with(
            name,
            tag,
            Some(authority_sk1),
            Some(Policy {
                owner: owner1,
                permissions: perms1,
            }),
        );

        for _ in 0..MAX_REG_NUM_ENTRIES {
            let (_hash, _op) = replica
                .write(random_register_entry(), BTreeSet::new())
                .context("Failed to write register entry")?;
        }

        let excess_entry = replica.write(random_register_entry(), BTreeSet::new());

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

    fn sign_register_op(mut op: RegisterOp<Entry>, sk: &SecretKey) -> Result<RegisterOp<Entry>> {
        let bytes = bincode::serialize(&op.crdt_op).map_err(|err| {
            warn!("We couldn't serialise the Register cmd: {err:?}");
            Error::RegisterCmdNotStored(op.address)
        })?;
        let signature = sk.sign(bytes);
        op.signature = Some(signature);
        Ok(op)
    }

    fn gen_reg_replicas(
        authority_sk: Option<SecretKey>,
        name: XorName,
        tag: u64,
        policy: Option<Policy>,
        count: usize,
    ) -> Vec<(SecretKey, RegisterReplica)> {
        let replicas: Vec<(SecretKey, RegisterReplica)> = (0..count)
            .map(|_| {
                let authority_sk = authority_sk.clone().unwrap_or_else(SecretKey::random);
                let authority = User::Key(authority_sk.public_key());
                let policy = policy.clone().unwrap_or_else(|| Policy {
                    owner: authority,
                    permissions: BTreeMap::new(),
                });
                let register = RegisterReplica::new(authority, name, tag, policy);
                (authority_sk, register)
            })
            .collect();

        assert_eq!(replicas.len(), count);
        replicas
    }

    fn create_reg_replicas(count: usize) -> Vec<(SecretKey, RegisterReplica)> {
        let name = xor_name::rand::random();
        let tag = 43_000;

        gen_reg_replicas(None, name, tag, None, count)
    }

    fn create_reg_replica_with(
        name: XorName,
        tag: u64,
        authority_sk: Option<SecretKey>,
        policy: Option<Policy>,
    ) -> RegisterReplica {
        let replicas = gen_reg_replicas(authority_sk, name, tag, policy, 1);
        replicas[0].1.clone()
    }

    // verify data convergence on a set of replicas and with the expected length
    fn verify_data_convergence(replicas: Vec<RegisterReplica>, expected_size: u64) -> Result<()> {
        // verify all replicas have the same and expected size
        for r in &replicas {
            assert_eq!(r.size(), expected_size);
        }

        // now verify that the items are the same in all replicas
        let r0 = &replicas[0];
        for r in &replicas {
            assert_eq!(r.crdt, r0.crdt);
        }

        Ok(())
    }

    // Generate a vec of Register replicas of some length, with corresponding vec of keypairs for signing, and the overall owner of the register
    fn generate_replicas(
        max_quantity: usize,
    ) -> impl Strategy<Value = Result<(Vec<RegisterReplica>, Arc<SecretKey>)>> {
        let xorname = xor_name::rand::random();
        let tag = 45_000u64;

        let owner_sk = Arc::new(SecretKey::random());
        let owner = User::Key(owner_sk.public_key());
        let policy = Policy {
            owner,
            permissions: BTreeMap::default(),
        };

        (1..max_quantity + 1).prop_map(move |quantity| {
            let mut replicas = Vec::with_capacity(quantity);
            for _ in 0..quantity {
                let replica = RegisterReplica::new(owner, xorname, tag, policy.clone());

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
            let name = xor_name::rand::random();
            let tag = 45_000u64;
            let owner_sk = SecretKey::random();
            let policy = Policy {
                owner: User::Key(owner_sk.public_key()),
                permissions: BTreeMap::default(),
            };

            let mut replicas = gen_reg_replicas(
                Some(owner_sk.clone()),
                name,
                tag,
                Some(policy),
                2);
            let (_, mut replica1) = replicas.remove(0);
            let (_, mut replica2) = replicas.remove(0);

            // Write an item on replicas
            let (_, op) = replica1.write(random_register_entry(), BTreeSet::new())?;
            let write_op = sign_register_op(op, &owner_sk)?;
            replica2.apply_op(write_op)?;

            verify_data_convergence(vec![replica1, replica2], 1)?;
        }

        #[test]
        fn proptest_reg_converge_with_many_random_data(
            dataset in generate_dataset(1000)
        ) {
            // Instantiate the same Register on two replicas
            let name = xor_name::rand::random();
            let tag = 43_000u64;
            let owner_sk = SecretKey::random();
            let policy = Policy {
                owner: User::Key(owner_sk.public_key()),
                permissions: BTreeMap::default(),
            };

            // Instantiate the same Register on two replicas
            let mut replicas = gen_reg_replicas(
                Some(owner_sk.clone()),
                name,
                tag,
                Some(policy),
                2);
            let (_, mut replica1) = replicas.remove(0);
            let (_, mut replica2) = replicas.remove(0);

            let dataset_length = dataset.len() as u64;

            // insert our data at replicas
            let mut children = BTreeSet::new();
            for _data in dataset {
                // Write an item on replica1
                let (hash, op) = replica1.write(random_register_entry(), children.clone())?;
                let write_op = sign_register_op(op, &owner_sk)?;
                // now apply that op to replica 2
                replica2.apply_op(write_op)?;
                children = vec![hash].into_iter().collect();
            }

            verify_data_convergence(vec![replica1, replica2], dataset_length)?;
        }

        #[test]
        fn proptest_reg_converge_with_many_random_data_random_entry_children(
            dataset in generate_dataset(1000)
        ) {
            // Instantiate the same Register on two replicas
            let name = xor_name::rand::random();
            let tag = 43_000u64;
            let owner_sk = SecretKey::random();
            let policy = Policy {
                owner: User::Key(owner_sk.public_key()),
                permissions: BTreeMap::default(),
            };

            // Instantiate the same Register on two replicas
            let mut replicas = gen_reg_replicas(
                Some(owner_sk.clone()),
                name,
                tag,
                Some(policy),
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
                let children: BTreeSet<_> = list_of_hashes.choose_multiple(&mut OsRng, num_of_children).cloned().collect();

                // Write an item on replica1 using the randomly generated set of children
                let (hash, op) = replica1.write(random_register_entry(), children)?;
                let write_op = sign_register_op(op, &owner_sk)?;

                // now apply that op to replica 2
                replica2.apply_op(write_op)?;
                list_of_hashes.push(hash);
            }

            verify_data_convergence(vec![replica1, replica2], dataset_length)?;
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
                let (hash, op)= replicas[0].write(random_register_entry(), children)?;
                let signed_op = sign_register_op(op, &owner_sk)?;

                // then apply this to all replicas
                for replica in &mut replicas {
                    replica.apply_op(signed_op.clone())?;
                }
                children = vec![hash].into_iter().collect();
            }

            verify_data_convergence(replicas, dataset_length)?;

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
                let (hash, op) = replicas[0].write(random_register_entry(), children)?;
                let signed_op = sign_register_op(op, &owner_sk)?;
                ops.push(signed_op);
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

            verify_data_convergence(replicas, dataset_length)?;
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
                    let (hash, op) = replica.write(random_register_entry(), children)?;
                    let signed_op = sign_register_op(op, &owner_sk)?;
                    ops.push(signed_op);
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

            verify_data_convergence(replicas, dataset_length)?;
        }

        #[test]
        fn proptest_dropped_data_can_be_reapplied_and_we_converge(
            dataset in generate_dataset_and_probability(1000),
        ) {
            // Instantiate the same Register on two replicas
            let name = xor_name::rand::random();
            let tag = 43_000u64;
            let owner_sk = SecretKey::random();
            let policy = Policy {
                owner: User::Key(owner_sk.public_key()),
                permissions: BTreeMap::default(),
            };

            // Instantiate the same Register on two replicas
            let mut replicas = gen_reg_replicas(
                Some(owner_sk.clone()),
                name,
                tag,
                Some(policy),
                2);
            let (_, mut replica1) = replicas.remove(0);
            let (_, mut replica2) = replicas.remove(0);

            let dataset_length = dataset.len() as u64;

            let mut ops = vec![];
            let mut children = BTreeSet::new();
            for (_data, delivery_chance) in dataset {
                let (hash, op)= replica1.write(random_register_entry(), children)?;
                let signed_op = sign_register_op(op, &owner_sk)?;

                ops.push((signed_op, delivery_chance));
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
            verify_data_convergence(vec![replica1, replica2], dataset_length)?;
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

                let (hash, op)=replica.write(random_register_entry(), children)?;
                let signed_op = sign_register_op(op, &owner_sk)?;
                ops.push((signed_op, delivery_chance));
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

            verify_data_convergence(replicas, dataset_length)?;
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
                    let (hash, op)=replica.write(random_register_entry(), children)?;
                    let signed_op = sign_register_op(op, &owner_sk)?;
                    ops.push(signed_op);
                    children = vec![hash].into_iter().collect();
                }
            }

            // set up a replica that has nothing to do with the rest, random xor... different owner...
            let xorname = xor_name::rand::random();
            let tag = 45_000u64;
            let random_owner_sk = SecretKey::random();
            let mut bogus_replica = RegisterReplica::new_owned(User::Key(random_owner_sk.public_key()), xorname, tag);

            // add bogus ops from bogus replica + bogus data
            let mut children = BTreeSet::new();
            for _data in bogus_dataset {
                let (hash, op)=bogus_replica.write(random_register_entry(), children)?;
                let bogus_op = sign_register_op(op, &random_owner_sk)?;
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

            verify_data_convergence(replicas, dataset_length as u64)?;
        }
    }

    fn random_register_entry() -> Vec<u8> {
        let random_bytes = thread_rng().gen::<[u8; 32]>();
        random_bytes.to_vec()
    }
}
