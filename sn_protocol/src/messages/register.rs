// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::storage::{
    registers::{DataAuthority, Entry, EntryHash, Policy, RegisterOp, User},
    RegisterAddress,
};

#[allow(unused_imports)] // needed by rustdocs links
use crate::{messages::QueryResponse, storage::registers::Register};

use serde::{Deserialize, Serialize};
use xor_name::XorName;

/// Register data exchange among replicas on the network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicatedRegisterLog {
    /// Register address
    pub address: RegisterAddress,
    /// Register ops log
    pub op_log: Vec<RegisterCmd>,
}

/// [`Register`] read operations.
#[derive(Hash, Eq, PartialEq, PartialOrd, Clone, Serialize, Deserialize, Debug)]
pub enum RegisterQuery {
    /// Retrieve the [`Register`] at the given address.
    ///
    /// This should eventually lead to a [`GetRegister`] response.
    ///
    /// [`GetRegister`]: QueryResponse::GetRegister
    Get(RegisterAddress),
    /// Retrieve the current entries from the [`Register`] at the given address.
    ///
    /// Multiple entries occur on concurrent writes. This should eventually lead to a
    /// [`ReadRegister`] response.
    ///
    /// [`ReadRegister`]: QueryResponse::ReadRegister
    Read(RegisterAddress),
    /// Get an entry from a [`Register`] on the Network by its hash
    ///
    /// This should eventually lead to a [`GetRegisterEntry`] response.
    ///
    /// [`GetRegisterEntry`]: QueryResponse::GetRegisterEntry
    GetEntry {
        /// Register address.
        address: RegisterAddress,
        /// The hash of the entry.
        hash: EntryHash,
    },
    /// Retrieve the policy of the [`Register`] at the given address.
    ///
    /// This should eventually lead to a [`GetRegisterPolicy`] response.
    ///
    /// [`GetRegisterPolicy`]: QueryResponse::GetRegisterPolicy
    GetPolicy(RegisterAddress),
    /// Retrieve the permissions of a given user for the [`Register`] at the given address.
    ///
    /// This should eventually lead to a [`GetRegisterUserPermissions`] response.
    ///
    /// [`GetRegisterUserPermissions`]: QueryResponse::GetRegisterUserPermissions
    GetUserPermissions {
        /// Register address.
        address: RegisterAddress,
        /// User to get permissions for.
        user: User,
    },
    /// Retrieve the owner of the [`Register`] at the given address.
    ///
    /// This should eventually lead to a [`GetRegisterOwner`] response.
    ///
    /// [`GetRegisterOwner`]: QueryResponse::GetRegisterOwner
    GetOwner(RegisterAddress),
}

/// A [`Register`] cmd that is stored in a log on Adults.
#[allow(clippy::large_enum_variant)]
#[derive(Eq, PartialEq, Clone, Serialize, Deserialize, Debug)]
pub enum RegisterCmd {
    /// Create a new [`Register`] on the network.
    Create(SignedRegisterCreate),
    /// Edit the [`Register`].
    Edit(SignedRegisterEdit),
}

///
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct CreateRegister {
    /// The name of the [`Register`].
    pub name: XorName,
    /// The tag on the [`Register`].
    pub tag: u64,
    /// The policy of the [`Register`].
    pub policy: Policy,
}

impl CreateRegister {
    /// Returns the owner of the register.
    pub fn owner(&self) -> User {
        self.policy.owner
    }

    /// Returns the address of the register.
    pub fn dst(&self) -> RegisterAddress {
        RegisterAddress {
            name: self.name,
            tag: self.tag,
        }
    }
}

///
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct EditRegister {
    /// The address of the [`Register`] to edit.
    pub address: RegisterAddress,
    /// The operation to perform.
    pub edit: RegisterOp<Entry>,
}

/// A signed cmd to create a [`Register`].
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct SignedRegisterCreate {
    /// Create a [`Register`].
    pub op: CreateRegister,
    /// A signature carrying authority to perform the operation.
    ///
    /// This will be verified against the Register's owner and permissions.
    pub auth: DataAuthority,
}

/// A [`Register`] write operation signed by the requester.
#[derive(Eq, PartialEq, Clone, Serialize, Deserialize, Debug)]
pub struct SignedRegisterEdit {
    /// The operation to perform.
    pub op: EditRegister,
    /// A signature carrying authority to perform the operation.
    ///
    /// This will be verified against the Register's owner and permissions.
    pub auth: DataAuthority,
}

impl SignedRegisterCreate {
    /// Returns the dst address of the register.
    pub fn dst(&self) -> RegisterAddress {
        self.op.dst()
    }
}

impl SignedRegisterEdit {
    /// Returns the dst address of the register.
    pub fn dst(&self) -> RegisterAddress {
        self.op.address
    }
}

impl RegisterQuery {
    /// Returns the dst address for the query.
    pub fn dst(&self) -> RegisterAddress {
        match self {
            Self::Get(ref address)
            | Self::Read(ref address)
            | Self::GetPolicy(ref address)
            | Self::GetUserPermissions { ref address, .. }
            | Self::GetEntry { ref address, .. }
            | Self::GetOwner(ref address) => *address,
        }
    }
}

impl RegisterCmd {
    /// Returns the name of the register.
    /// This is not a unique identifier.
    pub fn name(&self) -> XorName {
        *self.dst().name()
    }

    /// Returns the dst address of the register.
    pub fn dst(&self) -> RegisterAddress {
        match self {
            Self::Create(cmd) => cmd.dst(),
            Self::Edit(cmd) => cmd.dst(),
        }
    }
}
