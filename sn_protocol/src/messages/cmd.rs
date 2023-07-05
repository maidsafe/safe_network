// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::RegisterCmd;
use crate::{storage::DbcAddress, NetworkAddress};
use serde::{Deserialize, Serialize};
// TODO: remove this dependency and define these types herein.
pub use sn_dbc::{DbcId, DbcTransaction, Hash, SignedSpend};

/// Data and Dbc cmds - recording spends or creating, updating, and removing data.
///
/// See the [`protocol`] module documentation for more details of the types supported by the Safe
/// Network, and their semantics.
///
/// [`protocol`]: crate
#[allow(clippy::large_enum_variant)]
#[derive(Eq, PartialEq, Clone, Serialize, Deserialize, custom_debug::Debug)]
pub enum Cmd {
    /// [`Register`] write operation.
    ///
    /// [`Register`]: sn_registers::Register
    Register(RegisterCmd),
    /// [`SignedSpend`] write operation.
    ///
    /// [`SignedSpend`]: sn_dbc::SignedSpend
    /// The spend to be recorded
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
    /// Notify peer to send back a replication list
    ///
    /// [`NetworkAddress`]: crate::NetworkAddress
    RequestReplication(NetworkAddress),
}

impl Cmd {
    /// Used to send a cmd to the close group of the address.
    pub fn dst(&self) -> NetworkAddress {
        match self {
            Cmd::Register(cmd) => NetworkAddress::from_register_address(cmd.dst()),
            Cmd::SpendDbc(signed_spend) => {
                NetworkAddress::from_dbc_address(DbcAddress::from_dbc_id(signed_spend.dbc_id()))
            }
            Cmd::Replicate { holder, .. } => holder.clone(),
            Cmd::RequestReplication(sender) => sender.clone(),
        }
    }
}

impl std::fmt::Display for Cmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
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
            Cmd::RequestReplication(sender) => {
                write!(f, "Cmd::RequestReplication({:?})", sender.as_peer_id(),)
            }
        }
    }
}

// Data type of each of the nodes in the binary Merkle-tree built for payment proofs
pub type MerkleTreeNodesType = [u8; 32];

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, custom_debug::Debug)]
pub struct PaymentProof {
    // Ids of the DBCs spent, for nodes to check the storage payment is valid and inputs have
    // been effectivelly spent on the network.
    pub spent_ids: Vec<DbcId>,
    // Merkletree audit trail to prove the content storage has been paid by the
    // given DBC (using DBC's parent/s 'reason' field)
    pub audit_trail: Vec<MerkleTreeNodesType>,
    // Path of the audit trail
    pub path: Vec<usize>,
}
