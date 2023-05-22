// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod authority;
mod metadata;
mod policy;

pub use self::{
    authority::DataAuthority,
    metadata::{Action, Entry, EntryHash},
    policy::{Permissions, Policy, User},
};

use super::RegisterAddress;

use crdts::merkle_reg::{MerkleReg, Node};
use serde::{Deserialize, Serialize};

/// Content of a Register stored on the network
#[derive(Clone, Eq, PartialEq, PartialOrd, Hash, Serialize, Deserialize, Debug)]
pub struct Register {
    ///
    pub authority: User,
    ///
    pub crdt: RegisterCrdt,
    ///
    pub policy: Policy,
}

/// CRDT operation that can be applied to a Register
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd)]
pub struct RegisterCrdt {
    /// Address on the network of this piece of data
    pub address: RegisterAddress,
    /// CRDT to store the actual data, i.e. the items of the Register.
    // FIXME: MerkleReg perhaps shouldn't be a type from another crate but defined within this one.
    pub data: MerkleReg<Entry>,
}

/// Register mutation operation to apply to Register.
pub type RegisterOp<T> = CrdtOperation<T>;

/// CRDT Data operation applicable to other Register replica.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrdtOperation<T> {
    /// Address of a Register object on the network.
    pub address: RegisterAddress,
    /// The data operation to apply.
    // FIXME: Node perhaps shouldn't be a type from another crate but defined within this one.
    pub crdt_op: Node<T>,
    /// The PublicKey of the entity that generated the operation
    pub source: User,
    /// The signature of source on the crdt_top, required to apply the op
    pub signature: Option<bls::Signature>,
}
