// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use serde::{Deserialize, Serialize};
use sn_transfers::SpendAddress;
use thiserror::Error;

/// Errors that mean the DAG is invalid
#[derive(Error, Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Hash, PartialOrd, Ord)]
pub enum DagError {
    #[error("DAG has no source at {0:?}")]
    MissingSource(SpendAddress),
    #[error("DAG is incoherent at {0:?}: {1}")]
    IncoherentDag(SpendAddress, String),
    #[error("DAG with root {0:?} contains a cycle")]
    DagContainsCycle(SpendAddress),
}

/// List of possible faults that can be found in the DAG during verification
/// This indicates a certain spend is invalid and the reason for it
/// but does not mean the DAG is invalid
#[derive(Error, Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Hash, PartialOrd, Ord)]
pub enum SpendFault {
    #[error("Double Spend at {0:?}")]
    DoubleSpend(SpendAddress),
    #[error("Spend at {addr:?} has a missing ancestor at {ancestor:?}, until this ancestor is added to the DAG, it cannot be verified")]
    MissingAncestry {
        addr: SpendAddress,
        ancestor: SpendAddress,
    },
    #[error(
        "Spend at {addr:?} has a double spent ancestor at {ancestor:?}, making it unspendable"
    )]
    DoubleSpentAncestor {
        addr: SpendAddress,
        ancestor: SpendAddress,
    },
    #[error("Invalid transaction for spend at {0:?}: {1}")]
    InvalidTransaction(SpendAddress, String),
    #[error("Poisoned ancestry for spend at {0:?}: {1}")]
    PoisonedAncestry(SpendAddress, String),
    #[error("Spend at {addr:?} does not descend from given source: {src:?}")]
    OrphanSpend {
        addr: SpendAddress,
        src: SpendAddress,
    },
}

impl DagError {
    pub fn spend_address(&self) -> SpendAddress {
        match self {
            DagError::MissingSource(addr)
            | DagError::IncoherentDag(addr, _)
            | DagError::DagContainsCycle(addr) => *addr,
        }
    }
}

impl SpendFault {
    pub fn spend_address(&self) -> SpendAddress {
        match self {
            SpendFault::DoubleSpend(addr)
            | SpendFault::MissingAncestry { addr, .. }
            | SpendFault::DoubleSpentAncestor { addr, .. }
            | SpendFault::InvalidTransaction(addr, _)
            | SpendFault::PoisonedAncestry(addr, _)
            | SpendFault::OrphanSpend { addr, .. } => *addr,
        }
    }
}
