// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{Client, Error, Result};

use bls::PublicKey;
use libp2p::kad::Record;
use sn_protocol::{
    error::Error as ProtocolError,
    messages::RegisterCmd,
    storage::{try_serialize_record, RecordKind},
    NetworkAddress,
};
use sn_registers::{
    Entry, EntryHash, Permissions, Register, RegisterAddress, SignedRegister, User,
};

use std::collections::{BTreeSet, LinkedList};
use xor_name::XorName;

/// Ops made to an offline Register instance are applied locally only,
/// and accumulated till the user explicitly calls 'sync'. The user can
/// switch back to sync with the network for every op by invoking `online` API.
pub struct ClientRegister {
    client: Client,
    register: Register,
    ops: LinkedList<RegisterCmd>, // Cached operations.
}

impl ClientRegister {
    /// Central helper func to create a client register
    fn create_register(client: Client, meta: XorName, perms: Permissions) -> Result<Self> {
        let public_key = client.signer_pk();

        let register = Register::new(public_key, meta, perms);
        let reg = Self {
            client,
            register,
            ops: LinkedList::new(),
        };

        Ok(reg)
    }

    /// Create a new Register Locally.
    pub fn create(client: Client, meta: XorName) -> Result<Self> {
        Self::create_register(client, meta, Permissions::new_owner_only())
    }

    /// Create a new public Register (Anybody can write to it) and send it so the Network.
    /// This will optionally verify the Register was stored on the network.
    pub async fn create_public_online(
        client: Client,
        meta: XorName,
        verify_store: bool,
    ) -> Result<Self> {
        let mut reg = Self::create_register(client, meta, Permissions::new_owner_only())?;
        reg.sync(verify_store).await?;
        Ok(reg)
    }

    /// Create a new Register and send it to the Network.
    pub async fn create_online(client: Client, meta: XorName, verify_store: bool) -> Result<Self> {
        let mut reg = Self::create_register(client, meta, Permissions::new_owner_only())?;
        reg.sync(verify_store).await?;
        Ok(reg)
    }

    /// Retrieve a Register from the network to work on it offline.
    pub(super) async fn retrieve(client: Client, address: RegisterAddress) -> Result<Self> {
        let register = Self::get_register_from_network(&client, address).await?;

        Ok(Self {
            client,
            register,
            ops: LinkedList::new(),
        })
    }

    pub fn address(&self) -> &RegisterAddress {
        self.register.address()
    }

    /// Return the Owner of the Register.
    pub fn owner(&self) -> PublicKey {
        self.register.owner()
    }

    /// Return the Permissions of the Register.
    pub fn permissions(&self) -> &Permissions {
        self.register.permissions()
    }

    /// Return the number of items held in the register
    pub fn size(&self) -> u64 {
        self.register.size()
    }

    /// Return a value corresponding to the provided 'hash', if present.
    pub fn get(&self, hash: EntryHash) -> Result<&Entry> {
        let entry = self.register.get(hash)?;
        Ok(entry)
    }

    /// Read the last entry, or entries when there are branches, if the register is not empty.
    pub fn read(&self) -> BTreeSet<(EntryHash, Entry)> {
        self.register.read()
    }

    /// Write a new value onto the Register atop latest value.
    /// It returns an error if it finds branches in the content/entries; if it is
    /// required to merge/resolve the branches, invoke the `write_merging_branches` API.
    pub fn write(&mut self, entry: &[u8]) -> Result<()> {
        let children = self.register.read();
        if children.len() > 1 {
            return Err(Error::ContentBranchDetected(children));
        }

        self.write_atop(entry, children.into_iter().map(|(hash, _)| hash).collect())
    }

    /// Write a new value onto the Register atop latest value.
    /// If there are branches of content/entries, it automatically merges them
    /// all leaving the new value as a single latest value of the Register.
    /// Note you can use `write` API instead if you need to handle
    /// content/entries branches in a diffeerent way.
    pub fn write_merging_branches(&mut self, entry: &[u8]) -> Result<()> {
        let children: BTreeSet<EntryHash> = self
            .register
            .read()
            .into_iter()
            .map(|(hash, _)| hash)
            .collect();

        self.write_atop(entry, children)
    }

    /// Write a new value onto the Register atop the set of braches/entries
    /// referenced by the provided list of their corresponding entry hash.
    /// Note you can use `write_merging_branches` API instead if you
    /// want to write atop all exiting branches/entries.
    pub fn write_atop(&mut self, entry: &[u8], children: BTreeSet<EntryHash>) -> Result<()> {
        // check permissions first
        let public_key = self.client.signer_pk();
        self.register
            .check_user_permissions(User::Key(public_key))?;

        let (_hash, mut op) = self.register.write(entry.into(), children)?;
        let signature = self.client.sign(op.bytes_for_signing());
        op.add_signature(public_key, signature)?;
        let cmd = RegisterCmd::Edit(op);

        self.ops.push_front(cmd);

        Ok(())
    }

    // ********* Online methods  *********

    /// Sync this Register with the replicas on the network.
    /// This will optionally verify the stored Register on the network is the same as the local one.
    pub async fn sync(&mut self, verify_store: bool) -> Result<()> {
        debug!("Syncing Register at {:?}!", self.address());
        let remote_replica =
            match Self::get_register_from_network(&self.client, *self.address()).await {
                Ok(r) => r,
                Err(err) => {
                    debug!("Failed to fetch register: {err:?}");
                    debug!(
                        "Creating Register as it doesn't exist at {:?}!",
                        self.address(),
                    );
                    let cmd = RegisterCmd::Create {
                        register: self.register.clone(),
                        signature: self.client.sign(self.register.bytes()?),
                    };
                    self.publish_register(cmd, verify_store).await?;
                    self.register.clone()
                }
            };
        self.register.merge(remote_replica);
        self.push(verify_store).await
    }

    /// Push all operations made locally to the replicas of this Register on the network.
    /// This optionally verifies that the stored Register is the same as our local register
    pub async fn push(&mut self, verify_store: bool) -> Result<()> {
        let ops_len = self.ops.len();
        if ops_len > 0 {
            let address = *self.address();
            debug!("Pushing {ops_len} cached Register cmds at {address}!");

            // TODO: send them all concurrently
            while let Some(cmd) = self.ops.pop_back() {
                let result = self.publish_register(cmd.clone(), verify_store).await;

                if let Err(err) = result {
                    warn!("Did not push Register cmd on all nodes in the close group!: {err}");
                    // We keep the cmd for next sync to retry
                    self.ops.push_back(cmd);
                    return Err(err);
                }
            }

            debug!("Successfully pushed {ops_len} Register cmds at {address}!");
        }

        Ok(())
    }

    /// Write a new value onto the Register atop latest value.
    /// It returns an error if it finds branches in the content/entries; if it is
    /// required to merge/resolve the branches, invoke the `write_merging_branches` API.
    pub async fn write_online(&mut self, entry: &[u8], verify_store: bool) -> Result<()> {
        self.write(entry)?;
        self.push(verify_store).await
    }

    /// Write a new value onto the Register atop latest value.
    /// If there are branches of content/entries, it automatically merges them
    /// all leaving the new value as a single latest value of the Register.
    /// Note you can use `write` API instead if you need to handle
    /// content/entries branches in a diffeerent way.
    pub async fn write_merging_branches_online(
        &mut self,
        entry: &[u8],
        verify_store: bool,
    ) -> Result<()> {
        self.write_merging_branches(entry)?;
        self.push(verify_store).await
    }

    /// Write a new value onto the Register atop the set of braches/entries
    /// referenced by the provided list of their corresponding entry hash.
    /// Note you can use `write_merging_branches` API instead if you
    /// want to write atop all exiting branches/entries.
    pub async fn write_atop_online(
        &mut self,
        entry: &[u8],
        children: BTreeSet<EntryHash>,
        verify_store: bool,
    ) -> Result<()> {
        self.write_atop(entry, children)?;
        self.push(verify_store).await
    }

    // ********* Private helpers  *********

    /// Publish a `Register` command on the network.
    /// If `verify_store` is true, it will verify the Register was stored on the network.
    async fn publish_register(&self, cmd: RegisterCmd, verify_store: bool) -> Result<()> {
        let cmd_dst = cmd.dst();
        debug!("Querying existing Register for cmd: {cmd_dst:?}");
        let network_reg = self
            .client
            .get_signed_register_from_network(cmd.dst())
            .await;

        debug!("Publishing Register cmd: {cmd_dst:?}");
        let register = match cmd {
            RegisterCmd::Create {
                register,
                signature,
            } => {
                if let Ok(existing_reg) = network_reg {
                    if existing_reg.owner() != register.owner() {
                        return Err(ProtocolError::RegisterAlreadyClaimed(existing_reg.owner()))?;
                    }
                    return Ok(()); // no op, since already created
                }
                SignedRegister::new(register, signature)
            }
            RegisterCmd::Edit(op) => {
                let mut reg = network_reg?;
                reg.add_op(op)?;
                reg
            }
        };

        let key = NetworkAddress::from_register_address(*register.address()).to_record_key();

        let record = Record {
            key,
            value: try_serialize_record(&register, RecordKind::Register)?,
            publisher: None,
            expires: None,
        };

        // Register edits might exist so we cannot be sure that jsut because we get a record back that this should fail
        let expect_no_prior_record = false;
        Ok(self
            .client
            .network
            .put_record(record, verify_store, expect_no_prior_record)
            .await?)
    }

    // Retrieve a `Register` from the Network.
    async fn get_register_from_network(
        client: &Client,
        address: RegisterAddress,
    ) -> Result<Register> {
        debug!("Retrieving Register from: {address}");
        let reg = client.get_signed_register_from_network(address).await?;
        reg.verify_with_address(address)?;
        Ok(reg.register()?)
    }
}
