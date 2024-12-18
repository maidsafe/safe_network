// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{error::Result, Error, Permissions, RegisterAddress, RegisterOp};
#[cfg(feature = "test-utils")]
use bls::SecretKey;
use bls::{PublicKey, Signature};
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
    /// contains the info of meta (XorName) and owner (PublicKey)
    address: RegisterAddress,
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
    register: Register,
    /// signature over the above register by the owner
    signature: Signature,
    /// operations to apply on this register,
    /// they contain a signature of the writer
    ops: BTreeSet<RegisterOp>,
}

impl SignedRegister {
    /// Create a new SignedRegister
    pub fn new(register: Register, signature: Signature, ops: BTreeSet<RegisterOp>) -> Self {
        Self {
            register,
            signature,
            ops,
        }
    }

    /// Return the base register. This is the register before any operations have been applied.
    pub fn base_register(&self) -> &Register {
        &self.register
    }

    /// Verfies a SignedRegister
    pub fn verify(&self) -> Result<()> {
        let reg_size = self.ops.len();
        if reg_size >= MAX_REG_NUM_ENTRIES as usize {
            return Err(Error::TooManyEntries(reg_size));
        }

        let bytes = self.register.bytes()?;
        if !self
            .register
            .owner()
            .verify(&self.signature, bytes.as_slice())
        {
            return Err(Error::InvalidSignature);
        }

        for op in &self.ops {
            self.register.check_register_op(op)?;
            let size = op.crdt_op.value.len();
            if size > MAX_REG_ENTRY_SIZE {
                return Err(Error::EntryTooBig {
                    size,
                    max: MAX_REG_ENTRY_SIZE,
                });
            }
        }
        Ok(())
    }

    pub fn verify_with_address(&self, address: RegisterAddress) -> Result<()> {
        if self.register.address() != &address {
            return Err(Error::InvalidRegisterAddress {
                requested: Box::new(address),
                got: Box::new(*self.address()),
            });
        }
        self.verify()
    }

    /// Merge two SignedRegisters
    pub fn merge(&mut self, other: &Self) -> Result<()> {
        self.register.verify_is_mergeable(&other.register)?;
        self.ops.extend(other.ops.clone());
        Ok(())
    }

    /// Merge two SignedRegisters but verify the incoming content
    /// Significantly slower than merge, use when you want to trust but verify the `other`
    pub fn verified_merge(&mut self, other: &Self) -> Result<()> {
        self.register.verify_is_mergeable(&other.register)?;
        other.verify()?;
        self.ops.extend(other.ops.clone());
        Ok(())
    }

    /// Return the address.
    pub fn address(&self) -> &RegisterAddress {
        self.register.address()
    }

    /// Return the owner of the data.
    pub fn owner(&self) -> PublicKey {
        self.register.owner()
    }

    /// Check and add an Op to the SignedRegister
    pub fn add_op(&mut self, op: RegisterOp) -> Result<()> {
        let reg_size = self.ops.len();
        if reg_size >= MAX_REG_NUM_ENTRIES as usize {
            return Err(Error::TooManyEntries(reg_size));
        }

        let size = op.crdt_op.value.len();
        if size > MAX_REG_ENTRY_SIZE {
            return Err(Error::EntryTooBig {
                size,
                max: MAX_REG_ENTRY_SIZE,
            });
        }

        self.register.check_register_op(&op)?;
        self.ops.insert(op);
        Ok(())
    }

    /// Returns the reference to the ops list
    pub fn ops(&self) -> &BTreeSet<RegisterOp> {
        &self.ops
    }

    /// Used in tests.
    #[cfg(feature = "test-utils")]
    pub fn test_new_from_address(address: RegisterAddress, owner: &SecretKey) -> Self {
        let base_register = Register {
            address,
            permissions: Permissions::AnyoneCanWrite,
        };
        let bytes = if let Ok(bytes) = base_register.bytes() {
            bytes
        } else {
            panic!("Failed to serialize register {base_register:?}");
        };
        let signature = owner.sign(bytes);
        Self::new(base_register, signature, BTreeSet::new())
    }
}

impl Register {
    /// Create a new Register
    pub fn new(owner: PublicKey, meta: XorName, mut permissions: Permissions) -> Self {
        permissions.add_writer(owner);
        Self {
            address: RegisterAddress { meta, owner },
            permissions,
        }
    }

    /// Returns a bytes version of the Register used for signing
    /// Use this API when you want to sign a Register withtout providing a secret key to the Register API
    pub fn bytes(&self) -> Result<Vec<u8>> {
        rmp_serde::to_vec(self).map_err(|_| Error::SerialisationFailed)
    }

    /// Return the address.
    pub fn address(&self) -> &RegisterAddress {
        &self.address
    }

    /// Return the owner of the data.
    pub fn owner(&self) -> PublicKey {
        self.address.owner()
    }

    /// Return the permission.
    pub fn permissions(&self) -> &Permissions {
        &self.permissions
    }

    /// Check if a register op is valid for our current register
    pub fn check_register_op(&self, op: &RegisterOp) -> Result<()> {
        if self.permissions.can_anyone_write() {
            return Ok(()); // anyone can write, so no need to check the signature
        }
        self.check_user_permissions(op.source)?;
        op.verify_signature(&op.source)
    }

    /// Helper to check user write permissions for the given requester's public key.
    ///
    /// Returns:
    /// `Ok(())` if the user can write to this register
    /// `Err::AccessDenied` if the user cannot write to this register
    pub fn check_user_permissions(&self, requester: PublicKey) -> Result<()> {
        if self.permissions.can_write(&requester) {
            Ok(())
        } else {
            Err(Error::AccessDenied(requester))
        }
    }

    // Private helper to check if this Register is mergeable with another
    fn verify_is_mergeable(&self, other: &Self) -> Result<()> {
        if self.address() != other.address() || self.permissions != other.permissions {
            return Err(Error::DifferentBaseRegister);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{RegisterCrdt, RegisterOp};

    use super::*;

    use bls::SecretKey;
    use rand::{thread_rng, Rng};
    use std::collections::BTreeSet;
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
    fn register_permissions() -> eyre::Result<()> {
        let owner_sk = SecretKey::random();
        let owner = owner_sk.public_key();
        let user_sk_1 = SecretKey::random();
        let other_user = user_sk_1.public_key();
        let user_sk_2 = SecretKey::random();

        let meta: XorName = xor_name::rand::random();
        let address = RegisterAddress { meta, owner };

        // Create replicas where anyone can write to them, including the owner ofc
        let mut signed_reg_1 = create_reg_replica_with(
            meta,
            Some(owner_sk.clone()),
            Some(Permissions::new_anyone_can_write()),
        );
        // ...owner and any other users can both write to them
        let op = generate_random_op(address, &owner_sk)?;
        assert!(signed_reg_1.add_op(op).is_ok());
        let op = generate_random_op(address, &user_sk_1)?;
        assert!(signed_reg_1.add_op(op).is_ok());
        let op = generate_random_op(address, &user_sk_2)?;
        assert!(signed_reg_1.add_op(op).is_ok());

        // Create replicas allowing both the owner and other user to write to them
        let mut signed_reg_2 = create_reg_replica_with(
            meta,
            Some(owner_sk.clone()),
            Some(Permissions::new_with([other_user])),
        );
        // ...owner and the other user can both write to them, others shall fail
        let op = generate_random_op(address, &owner_sk)?;
        assert!(signed_reg_2.add_op(op).is_ok());
        let op = generate_random_op(address, &user_sk_1)?;
        assert!(signed_reg_2.add_op(op).is_ok());
        let op = generate_random_op(address, &user_sk_2)?;
        assert!(signed_reg_2.add_op(op).is_err());

        // Create replicas with the owner as the only allowed to write
        let mut signed_reg_3 = create_reg_replica_with(meta, Some(owner_sk.clone()), None);
        // ...owner can write to them
        let op = generate_random_op(address, &owner_sk)?;
        assert!(signed_reg_3.add_op(op).is_ok());
        // ...whilst other user cannot write to them
        let op = generate_random_op(address, &user_sk_1)?;
        let res = signed_reg_3.add_op(op);
        assert!(
            matches!(&res, Err(err) if err == &Error::AccessDenied(other_user)),
            "Unexpected result: {res:?}"
        );

        // Registers with different permission can not be merged
        let res1 = signed_reg_1.merge(&signed_reg_2);
        let res2 = signed_reg_2.merge(&signed_reg_1);
        assert!(
            matches!(&res1, Err(err) if err == &Error::DifferentBaseRegister),
            "Unexpected result: {res1:?}"
        );
        assert_eq!(res1, res2);

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
        assert_eq!(replica1.register.check_user_permissions(owner1), Ok(()));
        assert_eq!(replica1.register.check_user_permissions(owner2), Ok(()));
        assert_eq!(
            replica1.register.check_user_permissions(random_user),
            Ok(())
        );
        assert_eq!(
            replica1.register.check_user_permissions(random_user2),
            Ok(())
        );

        // check register 2 has only owner1 and owner2 write allowed
        assert_eq!(replica2.owner(), authority_pk2);
        assert_eq!(replica2.register.check_user_permissions(owner1), Ok(()));
        assert_eq!(replica2.register.check_user_permissions(owner2), Ok(()));
        assert_eq!(
            replica2.register.check_user_permissions(random_user),
            Err(Error::AccessDenied(random_user))
        );
        assert_eq!(
            replica2.register.check_user_permissions(random_user2),
            Err(Error::AccessDenied(random_user2))
        );

        Ok(())
    }

    #[test]
    fn exceeding_max_reg_entries_errors() -> eyre::Result<()> {
        let meta = xor_name::rand::random();

        // one replica will allow write ops to anyone
        let authority_sk1 = SecretKey::random();
        let owner = authority_sk1.public_key();
        let perms1 = Permissions::new_anyone_can_write();
        let address = RegisterAddress { meta, owner };

        let mut replica = create_reg_replica_with(meta, Some(authority_sk1.clone()), Some(perms1));

        for _ in 0..MAX_REG_NUM_ENTRIES {
            let op = generate_random_op(address, &authority_sk1)?;
            assert!(replica.add_op(op).is_ok());
        }

        let op = generate_random_op(address, &authority_sk1)?;

        let excess_entry = replica.add_op(op);

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
    ) -> Vec<(SecretKey, SignedRegister)> {
        let replicas: Vec<(SecretKey, SignedRegister)> = (0..count)
            .map(|_| {
                let authority_sk = authority_sk.clone().unwrap_or_else(SecretKey::random);
                let authority = authority_sk.public_key();
                let perms = perms.clone().unwrap_or_default();
                let register = Register::new(authority, meta, perms);

                let signature = authority_sk.sign(register.bytes().unwrap());
                let signed_reg = SignedRegister::new(register, signature, Default::default());

                (authority_sk, signed_reg)
            })
            .collect();

        assert_eq!(replicas.len(), count);
        replicas
    }

    fn create_reg_replica_with(
        meta: XorName,
        authority_sk: Option<SecretKey>,
        perms: Option<Permissions>,
    ) -> SignedRegister {
        let replicas = gen_reg_replicas(authority_sk, meta, perms, 1);
        replicas[0].1.clone()
    }

    fn random_register_entry() -> Vec<u8> {
        let random_bytes = thread_rng().gen::<[u8; 32]>();
        random_bytes.to_vec()
    }

    fn generate_random_op(address: RegisterAddress, writer_sk: &SecretKey) -> Result<RegisterOp> {
        let mut crdt_reg = RegisterCrdt::new(address);
        let item = random_register_entry();
        let (_hash, addr, crdt_op) = crdt_reg.write(item, &BTreeSet::new())?;
        Ok(RegisterOp::new(addr, crdt_op, writer_sk))
    }
}
