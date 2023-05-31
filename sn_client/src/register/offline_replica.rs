// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{Client, Error, Register, Result};

use bincode::serialize;
use sn_protocol::{
    messages::{
        Cmd, CmdResponse, CreateRegister, EditRegister, Query, QueryResponse, RegisterCmd,
        RegisterQuery, Request, Response, SignedRegisterCreate, SignedRegisterEdit,
    },
    storage::{
        registers::{Action, DataAuthority, Entry, EntryHash, Permissions, Policy, User},
        RegisterAddress,
    },
};
use sn_registers::RegisterReplica;
use std::{
    collections::{BTreeSet, LinkedList},
    convert::From,
};
use xor_name::XorName;

/// Ops made to an offline Register instance are applied locally only,
/// and accumulated till the user explicitly calls 'sync'. The user can
/// switch back to sync with the network for every op by invoking `online` API.
pub struct RegisterOffline {
    client: Client,
    register: RegisterReplica,
    ops: LinkedList<RegisterCmd>, // Cached operations.
}

impl RegisterOffline {
    /// Create a new Register offline.
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

    /// Instantiate a ReplicaOffline from a given Register instance.
    pub(super) fn from(replica: Register) -> Self {
        Self {
            client: replica.offline_reg.client,
            register: replica.offline_reg.register,
            ops: LinkedList::new(),
        }
    }

    /// Switch to 'online' mode where each op made locally is immediatelly pushed to the network.
    pub async fn online(mut self) -> Result<Register> {
        self.push().await?;
        Ok(Register { offline_reg: self })
    }

    /// Return the Policy of the Register.
    pub fn policy(&self) -> &Policy {
        self.register.policy()
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
        // we need to check permissions first
        let public_key = self.client.signer_pk();
        self.register
            .check_permissions(Action::Write, Some(User::Key(public_key)))?;

        let (_hash, edit) = self.register.write(entry.into(), children)?;
        let op = EditRegister {
            address: *self.register.address(),
            edit,
        };
        let auth = DataAuthority {
            public_key,
            signature: self.client.sign(&serialize(&op)?),
        };
        let cmd = RegisterCmd::Edit(SignedRegisterEdit { op, auth });

        self.ops.push_front(cmd);

        Ok(())
    }

    /// Sync this Register with the replicas on the network.
    pub async fn sync(&mut self) -> Result<()> {
        debug!("Syncing Register at {}, {}!", self.name(), self.tag(),);
        // FIXME: handle the scenario where the Register doesn't exist on the network yet
        let remote_replica = Self::get_register(&self.client, *self.name(), self.tag()).await?;
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
                let result = match cmd {
                    RegisterCmd::Create { .. } => self.publish_register_create(cmd.clone()).await,
                    RegisterCmd::Edit { .. } => self.publish_register_edit(cmd.clone()).await,
                };

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

    // ********* Private helpers  *********

    // Create a new RegisterOffline instance with the given name and tag.
    fn new(client: Client, name: XorName, tag: u64) -> Result<Self> {
        let public_key = client.signer_pk();
        let owner = User::Key(public_key);
        let policy = Policy {
            owner,
            permissions: [(User::Anyone, Permissions::new(true))]
                .into_iter()
                .collect(),
        };

        let op = CreateRegister {
            name,
            tag,
            policy: policy.clone(),
        };
        let auth = DataAuthority {
            public_key,
            signature: client.sign(&serialize(&op)?),
        };
        let create_cmd = RegisterCmd::Create(SignedRegisterCreate { op, auth });

        let register = RegisterReplica::new(owner, name, tag, policy);
        let reg = Self {
            client,
            register,
            ops: LinkedList::from([create_cmd]),
        };

        Ok(reg)
    }

    // Publish a `Register` creation command on the network.
    async fn publish_register_create(&self, cmd: RegisterCmd) -> Result<()> {
        debug!("Publishing Register create cmd: {:?}", cmd.dst());
        let request = Request::Cmd(Cmd::Register(cmd));
        let responses = self.client.send_to_closest(request).await?;

        let all_ok = responses
            .iter()
            .all(|resp| matches!(resp, Ok(Response::Cmd(CmdResponse::CreateRegister(Ok(()))))));
        if all_ok {
            return Ok(());
        }

        // If not all were Ok, we will return the first error sent to us.
        for resp in responses.iter().flatten() {
            if let Response::Cmd(CmdResponse::CreateRegister(result)) = resp {
                result.clone()?;
            };
        }

        // If there were no success or fail to the expected query,
        // we check if there were any send errors.
        for resp in responses {
            let _ = resp?;
        }

        // If there were no register errors, then we had unexpected responses.
        Err(Error::UnexpectedResponses)
    }

    // Publish a `Register` edit command in the network.
    async fn publish_register_edit(&self, cmd: RegisterCmd) -> Result<()> {
        debug!("Publishing Register edit cmd: {:?}", cmd.dst());
        let request = Request::Cmd(Cmd::Register(cmd));
        let responses = self.client.send_to_closest(request).await?;

        let all_ok = responses
            .iter()
            .all(|resp| matches!(resp, Ok(Response::Cmd(CmdResponse::EditRegister(Ok(()))))));
        if all_ok {
            return Ok(());
        }

        // If not all were Ok, we will return the first error sent to us.
        for resp in responses.iter().flatten() {
            if let Response::Cmd(CmdResponse::EditRegister(result)) = resp {
                result.clone()?;
            };
        }

        // If there were no success or fail to the expected query,
        // we check if there were any send errors.
        for resp in responses {
            let _ = resp?;
        }

        // If there were no register errors, then we had unexpected responses.
        Err(Error::UnexpectedResponses)
    }

    // Retrieve a `Register` from the closest peers.
    async fn get_register(client: &Client, name: XorName, tag: u64) -> Result<RegisterReplica> {
        let address = RegisterAddress { name, tag };
        debug!("Retrieving Register from: {address:?}");
        let request = Request::Query(Query::Register(RegisterQuery::Get(address)));
        let responses = client.send_to_closest(request).await?;

        // We will return the first register we get.
        for resp in responses.iter().flatten() {
            if let Response::Query(QueryResponse::GetRegister(Ok(register))) = resp {
                return Ok(register.clone().into());
            };
        }

        // If no register was gotten, we will return the first error sent to us.
        for resp in responses.iter().flatten() {
            if let Response::Query(QueryResponse::GetRegister(result)) = resp {
                let _ = result.clone()?;
            };
        }

        // If there were no success or fail to the expected query,
        // we check if there were any send errors.
        for resp in responses {
            let _ = resp?;
        }

        // If there was none of the above, then we had unexpected responses.
        Err(Error::UnexpectedResponses)
    }
}
