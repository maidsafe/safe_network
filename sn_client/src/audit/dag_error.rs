// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sn_transfers::SpendAddress;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum DagError {
    // Errors that mean the DAG is invalid
    #[error("DAG has no valid source at {0:?}")]
    MissingSource(SpendAddress),
    #[error("DAG is incoherent at {0:?}: {1}")]
    IncoherentDag(SpendAddress, String),

    // Errors that mean a Spend in the DAG is invalid
    #[error("Double Spend at {0:?}")]
    DoubleSpend(SpendAddress),
    #[error("Spend at {0:?} has missing ancestors")]
    MissingAncestry(SpendAddress),
    #[error("Invalid transaction for spend at {0:?}: {1}")]
    InvalidTransaction(SpendAddress, String),
    #[error("Poisoned ancestry for spend at {0:?}: {1}")]
    PoisonedAncestry(SpendAddress, String),
    #[error("Spend at {orphan:?} does not descend from given source: {src:?}")]
    OrphanSpend {
        orphan: SpendAddress,
        src: SpendAddress,
    },
}

impl DagError {
    pub fn dag_is_invalid(&self) -> bool {
        matches!(
            self,
            DagError::MissingSource(_)
            | DagError::IncoherentDag(_, _)
        )
    }
}