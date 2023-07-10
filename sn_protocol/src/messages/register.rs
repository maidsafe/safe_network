// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sn_registers::{Permissions, RegisterAddress, RegisterOp, User};

#[allow(unused_imports)] // needed by rustdocs links
use crate::messages::QueryResponse;
#[allow(unused_imports)] // needed by rustdocs links
use sn_registers::Register;

use serde::{Deserialize, Serialize};
use xor_name::XorName;

/// A [`Register`] cmd that is stored in a log on Adults.
#[allow(clippy::large_enum_variant)]
#[derive(Eq, PartialEq, Clone, Serialize, Deserialize, Debug)]
pub enum RegisterCmd {
    /// Create a new [`Register`] on the network.
    Create {
        /// The owner of the register
        owner: User,
        /// The name of the register
        name: XorName,
        /// The tag on the register
        tag: u64,
        /// The permissions of the register
        permissions: Permissions,
    },
    /// Edit the [`Register`].
    Edit(RegisterOp),
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
            Self::Create { name, tag, .. } => RegisterAddress {
                name: *name,
                tag: *tag,
            },
            Self::Edit(op) => op.address(),
        }
    }
}
