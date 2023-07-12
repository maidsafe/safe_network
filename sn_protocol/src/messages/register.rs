// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sn_registers::{Register, RegisterAddress, RegisterOp};

use serde::{Deserialize, Serialize};
use xor_name::XorName;

/// A [`Register`] cmd that is stored in a log on Adults.
#[allow(clippy::large_enum_variant)]
#[derive(Eq, PartialEq, Clone, Serialize, Deserialize, Debug)]
pub enum RegisterCmd {
    /// Create a new [`Register`] on the network.
    Create {
        /// The base register (contains, owner, name, tag, permissions, and register initial state)
        register: Register,
        /// The signature of the owner on that register.
        signature: bls::Signature,
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
            Self::Create { register, .. } => *register.address(),
            Self::Edit(op) => op.address(),
        }
    }
}
