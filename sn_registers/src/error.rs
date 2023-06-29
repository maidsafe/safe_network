// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{EntryHash, RegisterAddress, User};

#[derive(Error, Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum Error {
    /// Register operation destination address mismatch
    #[error(
        "The CRDT operation cannot be applied since the Register operation destination address ({dst_addr:?}) \
         doesn't match the targeted Register's address: {reg_addr:?}"
    )]
    RegisterAddrMismatch {
        /// Register operation destination address
        dst_addr: RegisterAddress,
        /// Targeted Register's address
        reg_addr: RegisterAddress,
    },
    /// Entry is too big to fit inside a register
    #[error("Entry is too big to fit inside a register: {size}, max: {max}")]
    EntryTooBig {
        /// Size of the entry
        size: usize,
        /// Maximum entry size allowed
        max: usize,
    },
    /// Access denied for user
    #[error("Access denied for user: {0:?}")]
    AccessDenied(User),
    /// Cannot add another entry since the register entry cap has been reached.
    #[error("Cannot add another entry since the register entry cap has been reached: {0}")]
    TooManyEntries(usize),
    /// Entry could not be found on the data
    #[error("Requested entry not found {0}")]
    NoSuchEntry(EntryHash),
    /// Serialisation Failed
    #[error("Serialisation failed")]
    SerialisationFailed,
    /// Invalid Signature found in register op
    #[error("Invalid signature")]
    InvalidSignature,
    /// Missing Signature when expecting one in register op
    #[error("Missing signature")]
    MissingSignature,
}

pub(crate) type Result<T> = std::result::Result<T, Error>;
