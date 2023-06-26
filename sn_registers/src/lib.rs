// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod address;
mod authority;
pub(crate) mod error;
mod metadata;
mod policy;
pub(crate) mod reg_crdt;
pub(crate) mod register;

pub use self::{
    address::RegisterAddress,
    authority::DataAuthority,
    error::Error,
    metadata::{Action, Entry, EntryHash},
    policy::{Permissions, Policy, User},
    register::{Register, RegisterOp},
};
