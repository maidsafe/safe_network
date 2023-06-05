// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    storage::{Chunk, ChunkAddress, DbcAddress},
    NetworkAddress,
};

use super::RegisterCmd;

// TODO: remove this dependency and define these types herein.
pub use sn_dbc::{Hash, SignedSpend};

use serde::{Deserialize, Serialize};

/// Data and Dbc cmds - recording spends or creating, updating, and removing data.
///
/// See the [`protocol`] module documentation for more details of the types supported by the Safe
/// Network, and their semantics.
///
/// [`protocol`]: crate
#[allow(clippy::large_enum_variant)]
#[derive(Eq, PartialEq, Clone, Serialize, Deserialize, custom_debug::Debug)]
pub enum Cmd {
    /// [`Chunk`] write operation.
    ///
    /// [`Chunk`]: crate::storage::Chunk
    StoreChunk {
        chunk: Chunk,
        // Storage payment proof
        // TODO: temporarily payment proof is optional
        payment: Option<PaymentProof>,
    },
    /// [`Register`] write operation.
    ///
    /// [`Register`]: crate::storage::Register
    Register(RegisterCmd),
    /// [`SignedSpend`] write operation.
    ///
    /// [`SignedSpend`]: sn_dbc::SignedSpend
    /// The spend to be recorded.
    /// It contains the transaction it is being spent in.
    SpendDbc(SignedSpend),
    /// Write operation to notify peer fetch a list of [`NetworkAddress`] from the holder.
    ///
    /// [`NetworkAddress`]: crate::NetworkAddress
    Replicate {
        /// Holder of the replication keys.
        holder: NetworkAddress,
        /// Keys of copy that shall be replicated.
        #[debug(skip)]
        keys: Vec<NetworkAddress>,
    },
}

impl Cmd {
    /// Used to send a cmd to the close group of the address.
    pub fn dst(&self) -> NetworkAddress {
        match self {
            Cmd::StoreChunk { chunk, .. } => {
                NetworkAddress::from_chunk_address(ChunkAddress::new(*chunk.name()))
            }
            Cmd::Register(cmd) => NetworkAddress::from_register_address(cmd.dst()),
            Cmd::SpendDbc(signed_spend) => {
                NetworkAddress::from_dbc_address(DbcAddress::from_dbc_id(signed_spend.dbc_id()))
            }
            Cmd::Replicate { holder, .. } => holder.clone(),
        }
    }
}

impl std::fmt::Display for Cmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Cmd::StoreChunk { chunk, .. } => {
                write!(f, "Cmd::StoreChunk({:?})", chunk.name())
            }
            Cmd::Register(cmd) => {
                write!(f, "Cmd::Register({:?})", cmd.name()) // more qualification needed
            }
            Cmd::SpendDbc(signed_spend) => {
                write!(f, "Cmd::SpendDbc({:?})", signed_spend.dbc_id())
            }
            Cmd::Replicate { holder, keys } => {
                write!(
                    f,
                    "Cmd::Replicate({:?} has {} keys)",
                    holder.as_peer_id(),
                    keys.len()
                )
            }
        }
    }
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, custom_debug::Debug)]
pub struct PaymentProof {
    // Reason-hash value set in the input/parent DBCs spent for this storage payment.
    // TOOD: pass the output DBC instead, nodes can check input/parent DBCs' reason-hash among other pending validations.
    pub reason_hash: Hash,
    // Merkletree audit trail to prove the Chunk has been paid by the
    // given DBC (using the DBC's 'reason' field)
    pub lemma: Vec<[u8; 32]>,
    // Path of the audit trail
    pub path: Vec<usize>,
}
