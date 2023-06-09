// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{Client, Error, Result};

use libp2p::kad::{Record, RecordKey};
use sn_protocol::{
    messages::RegisterCmd,
    storage::{try_serialize_record, RecordKind},
};
use sn_registers::{Entry, EntryHash, Permissions, Register, RegisterAddress, User};

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
    /// Create a new Register.
    pub fn create(client: Client, name: XorName, tag: u64) -> Result<Self> {
        Self::new(client, name, tag)
    }

    /// Retrieve a Register from the network to work on it offline.
    pub(super) async fn retrieve(client: Client, name: XorName, tag: u64) -> Result<Self> {
        let register = Self::get_register(&client, name, tag).await?;

        Ok(Self {
            client,
            register,
            ops: LinkedList::new(),
        })
    }

    /// Return the Owner of the Register.
    pub fn owner(&self) -> User {
        self.register.owner()
    }

    /// Return the Permissions of the Register.
    pub fn permissions(&self) -> &Permissions {
        self.register.permissions()
    }

    /// Return the XorName of the Register.
    pub fn name(&self) -> &XorName {
        self.register.name()
    }

    /// Return the tag value of the Register.
    pub fn tag(&self) -> u64 {
        self.register.tag()
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

        let (_hash, op) = self.register.write(entry.into(), children)?;
        let cmd = RegisterCmd::Edit(op);

        self.ops.push_front(cmd);

        Ok(())
    }

    // ********* Online methods  *********

    /// Sync this Register with the replicas on the network.
    pub async fn sync(&mut self) -> Result<()> {
        debug!("Syncing Register at {}, {}!", self.name(), self.tag());
        let remote_replica = match Self::get_register(&self.client, *self.name(), self.tag()).await
        {
            Ok(r) => r,
            Err(err) => {
                debug!("Failed to fetch register: {err:?}");
                debug!(
                    "Creating Register as it doesn't exist at {}, {}!",
                    self.name(),
                    self.tag()
                );
                let cmd = RegisterCmd::Create {
                    owner: self.owner(),
                    permissions: self.permissions().clone(),
                    name: self.name().to_owned(),
                    tag: self.tag(),
                };
                self.publish_register(cmd).await?;
                self.register.clone()
            }
        };
        self.register.merge(remote_replica);
        self.push().await
    }

    /// Push all operations made locally to the replicas of this Register on the network.
    pub async fn push(&mut self) -> Result<()> {
        let ops_len = self.ops.len();
        if ops_len > 0 {
            let name = *self.name();
            let tag = self.tag();
            debug!("Pushing {ops_len} cached Register cmds at {name}, {tag}!",);

            // TODO: send them all concurrently
            while let Some(cmd) = self.ops.pop_back() {
                let result = self.publish_register(cmd.clone()).await;

                if let Err(err) = result {
                    warn!("Did not push Register cmd on all nodes in the close group!: {err}");
                    // We keep the cmd for next sync to retry
                    self.ops.push_back(cmd);
                    return Err(err);
                }
            }

            debug!("Successfully pushed {ops_len} Register cmds at {name}, {tag}!",);
        }

        Ok(())
    }

    /// Write a new value onto the Register atop latest value.
    /// It returns an error if it finds branches in the content/entries; if it is
    /// required to merge/resolve the branches, invoke the `write_merging_branches` API.
    pub async fn write_online(&mut self, entry: &[u8]) -> Result<()> {
        self.write(entry)?;
        self.push().await
    }

    /// Write a new value onto the Register atop latest value.
    /// If there are branches of content/entries, it automatically merges them
    /// all leaving the new value as a single latest value of the Register.
    /// Note you can use `write` API instead if you need to handle
    /// content/entries branches in a diffeerent way.
    pub async fn write_merging_branches_online(&mut self, entry: &[u8]) -> Result<()> {
        self.write_merging_branches(entry)?;
        self.push().await
    }

    /// Write a new value onto the Register atop the set of braches/entries
    /// referenced by the provided list of their corresponding entry hash.
    /// Note you can use `write_merging_branches` API instead if you
    /// want to write atop all exiting branches/entries.
    pub async fn write_atop_online(
        &mut self,
        entry: &[u8],
        children: BTreeSet<EntryHash>,
    ) -> Result<()> {
        self.write_atop(entry, children)?;
        self.push().await
    }

    // ********* Private helpers  *********

    // Create a new `ClientRegister` instance with the given name and tag.
    fn new(client: Client, name: XorName, tag: u64) -> Result<Self> {
        let public_key = client.signer_pk();
        let owner = User::Key(public_key);
        let perms = Permissions::new_anyone_can_write();

        let register = Register::new(owner, name, tag, perms);
        let reg = Self {
            client,
            register,
            ops: LinkedList::new(),
        };

        Ok(reg)
    }

    // Publish a `Register` command on the network.
    async fn publish_register(&self, cmd: RegisterCmd) -> Result<()> {
        let cmd_dst = cmd.dst();
        debug!("Querying existing Register for cmd: {cmd_dst:?}");

        let maybe_register = self.client.get_register_from_network(cmd.dst()).await;

        debug!("Publishing Register cmd: {cmd_dst:?}");

        let register = match cmd {
            RegisterCmd::Create {
                owner,
                name,
                tag,
                permissions,
            } => {
                if maybe_register.is_ok() {
                    // no op, since already created
                    return Ok(());
                }
                Register::new(owner, name, tag, permissions)
            }
            RegisterCmd::Edit(op) => {
                if let Ok(mut register) = maybe_register {
                    register.apply_op(op)?;
                    register
                } else {
                    warn!("Cann't fetch register for RegisterEdit cmd {cmd_dst:?}");
                    return Err(Error::UnexpectedResponses);
                }
            }
        };

        let reg_addr = register.address();
        let record = Record {
            key: RecordKey::new(reg_addr.name()),
            value: try_serialize_record(&register, RecordKind::Register)?,
            publisher: None,
            expires: None,
        };
        Ok(self.client.network.put_record(record).await?)
    }

    // Retrieve a `Register` from the closest peers.
    async fn get_register(client: &Client, name: XorName, tag: u64) -> Result<Register> {
        let address = RegisterAddress { name, tag };
        debug!("Retrieving Register from: {address:?}");
        client.get_register_from_network(address).await
    }
}
