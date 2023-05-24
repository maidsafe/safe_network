// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod offline_replica;

pub use offline_replica::RegisterOffline;

use super::{
    error::{Error, Result},
    Client,
};

use sn_protocol::storage::registers::{Entry, EntryHash, Policy};
use std::collections::BTreeSet;
use xor_name::XorName;

/// Operations made to a Register instance are applied not only locally,
/// but also sent them to the replicas on the network. The user can
/// switch offline-mode by invoking the `offline` API.
pub struct Register {
    pub(super) offline_reg: RegisterOffline,
}

impl Register {
    /// Create a new Register.
    pub async fn create(client: Client, name: XorName, tag: u64) -> Result<Self> {
        let mut offline_reg = RegisterOffline::create(client, name, tag)?;
        offline_reg.push().await?;
        Ok(Self { offline_reg })
    }

    /// Retrieve a Register from the network.
    pub async fn retrieve(client: Client, name: XorName, tag: u64) -> Result<Self> {
        let offline_reg = RegisterOffline::retrieve(client, name, tag).await?;
        Ok(Self { offline_reg })
    }

    /// Switch to 'offline' mode where each op is made only locally.
    pub fn offline(self) -> RegisterOffline {
        RegisterOffline::from(self)
    }

    /// Return the Policy of the Register.
    pub fn policy(&self) -> &Policy {
        self.offline_reg.policy()
    }

    /// Return the XorName of the Register.
    pub fn name(&self) -> &XorName {
        self.offline_reg.name()
    }

    /// Return the tag value of the Register.
    pub fn tag(&self) -> u64 {
        self.offline_reg.tag()
    }

    /// Return the number of items held in the register
    pub fn size(&self) -> u64 {
        self.offline_reg.size()
    }

    /// Return a value corresponding to the provided 'hash', if present.
    pub fn get(&self, hash: EntryHash) -> Result<&Entry> {
        self.offline_reg.get(hash)
    }

    /// Read the last entry, or entries when there are branches, if the register is not empty.
    pub fn read(&self) -> BTreeSet<(EntryHash, Entry)> {
        self.offline_reg.read()
    }

    /// Write a new value onto the Register atop latest value.
    /// It returns an error if it finds branches in the content/entries; if it is
    /// required to merge/resolve the branches, invoke the `write_merging_branches` API.
    pub async fn write(&mut self, entry: &[u8]) -> Result<()> {
        self.offline_reg.write(entry)?;
        self.offline_reg.push().await
    }

    /// Write a new value onto the Register atop latest value.
    /// If there are branches of content/entries, it automatically merges them
    /// all leaving the new value as a single latest value of the Register.
    /// Note you can use `write` API instead if you need to handle
    /// content/entries branches in a diffeerent way.
    pub async fn write_merging_branches(&mut self, entry: &[u8]) -> Result<()> {
        self.offline_reg.write_merging_branches(entry)?;
        self.offline_reg.push().await
    }

    /// Write a new value onto the Register atop the set of braches/entries
    /// referenced by the provided list of their corresponding entry hash.
    /// Note you can use `write_merging_branches` API instead if you
    /// want to write atop all exiting branches/entries.
    pub async fn write_atop(&mut self, entry: &[u8], children: BTreeSet<EntryHash>) -> Result<()> {
        self.offline_reg.write_atop(entry, children)?;
        self.offline_reg.push().await
    }

    /// Sync this Register with the replicas on the network.
    pub async fn sync(&mut self) -> Result<()> {
        self.offline_reg.sync().await
    }
}
