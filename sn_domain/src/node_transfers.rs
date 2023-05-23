// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::dbc_genesis::GENESIS_DBC;

use crate::{dbc_genesis::is_genesis_parent_tx, storage::SpendStorage};

use sn_dbc::{DbcTransaction, SignedSpend};
use sn_protocol::{error::TransferError, storage::DbcAddress};

use std::{collections::BTreeSet, path::Path};

// Result for all related to node handling of transfers.
type Result<T> = std::result::Result<T, TransferError>;

pub struct Transfers {
    storage: SpendStorage,
}

impl Transfers {
    /// Create a new instance of `Transfers`.
    pub fn new(root_dir: &Path) -> Self {
        Self {
            storage: SpendStorage::new(root_dir),
        }
    }

    /// Get Spend from local store.
    pub async fn get(&self, address: DbcAddress) -> Result<SignedSpend> {
        Ok(self.storage.get(&address).await?)
    }

    /// Tries to add a double spend that was detected by the network.
    pub async fn try_add_double(
        &mut self,
        a_spend: &SignedSpend,
        b_spend: &SignedSpend,
    ) -> Result<()> {
        Ok(self.storage.try_add_double(a_spend, b_spend).await?)
    }

    /// Tries to add a new spend to the queue.
    ///
    /// All the provided data will be validated, and
    /// if it is valid, the spend will be pushed onto the queue.
    pub async fn try_add(
        &mut self,
        signed_spend: Box<SignedSpend>,
        parent_tx: Box<DbcTransaction>,
        parent_spends: BTreeSet<SignedSpend>,
    ) -> Result<()> {
        // 1. Validate the tx hash.
        // Ensure that the provided src tx is the same as the
        // one we have the hash of in the signed spend.
        let provided_src_tx_hash = parent_tx.hash();
        let signed_src_tx_hash = signed_spend.src_tx_hash();

        if provided_src_tx_hash != signed_src_tx_hash {
            return Err(TransferError::TxSourceMismatch {
                signed_src_tx_hash,
                provided_src_tx_hash,
            });
        }

        // 3. Validate the spend itself.
        self.storage.validate(signed_spend.as_ref()).await?;

        // 4. Validate the parents of the spend.
        // This also ensures that all parent's dst tx's are the same as the src tx of this spend.
        validate_parent_spends(signed_spend.as_ref(), parent_tx.as_ref(), parent_spends)?;

        match self.storage.try_add(&signed_spend).await {
            Ok(true) => {
                trace!("Added popped spend to storage.");
            }
            Ok(false) => {
                trace!("Spend already existed in storage. Nothing added.");
            }
            Err(e) => {
                trace!("Could not add popped spend to storage. Dropping it. Error: {e}.");
            }
        }

        Ok(())
    }
}

/// The src_tx is the tx where the dbc to spend, was created.
/// The signed_spend.dbc_id() shall exist among its outputs.
fn validate_parent_spends(
    signed_spend: &SignedSpend,
    parent_tx: &DbcTransaction,
    parent_spends: BTreeSet<SignedSpend>,
) -> Result<()> {
    trace!("Validating parent spends..");
    // The parent_spends will be different spends,
    // one for each input that went into creating the signed_spend.
    for parent_spend in &parent_spends {
        // The dst tx of the parent must be the src tx of the spend.
        if signed_spend.src_tx_hash() != parent_spend.dst_tx_hash() {
            return Err(TransferError::TxTrailMismatch {
                signed_src_tx_hash: signed_spend.src_tx_hash(),
                parent_dst_tx_hash: parent_spend.dst_tx_hash(),
            });
        }
    }

    // We have gotten all the parent inputs from the network, so the network consider them all valid.
    // But the source tx corresponding to the signed_spend, might not match the parents' details, so that's what we check here.
    let known_parent_blinded_amounts: Vec<_> = parent_spends
        .iter()
        .map(|s| s.spend.blinded_amount)
        .collect();

    if is_genesis_parent_tx(parent_tx) && signed_spend.dbc_id() == &GENESIS_DBC.id {
        return Ok(());
    }

    // Here we check that the spend that is attempted, was created in a valid tx.
    let src_tx_validity = parent_tx.verify(&known_parent_blinded_amounts);
    if src_tx_validity.is_err() {
        return Err(TransferError::InvalidSourceTxProvided {
            signed_src_tx_hash: signed_spend.src_tx_hash(),
            provided_src_tx_hash: parent_tx.hash(),
        });
    }

    trace!("All parents check out.");

    Ok(())
}
