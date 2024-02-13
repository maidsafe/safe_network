// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};

/// An entry in a Register (note that the `vec<u8>` is size limited: `MAX_REG_ENTRY_SIZE`)
pub type Entry = Vec<u8>;

/// Hash of the register entry. Logging as the same format of `XorName`.
#[derive(Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntryHash(pub crdts::merkle_reg::Hash);

impl Debug for EntryHash {
    fn fmt(&self, formatter: &mut Formatter) -> FmtResult {
        write!(formatter, "{self}")
    }
}

impl Display for EntryHash {
    fn fmt(&self, formatter: &mut Formatter) -> FmtResult {
        write!(
            formatter,
            "{:02x}{:02x}{:02x}..",
            self.0[0], self.0[1], self.0[2]
        )
    }
}
