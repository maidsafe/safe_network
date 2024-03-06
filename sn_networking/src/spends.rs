// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{Error, Network, Result};
use futures::future::join_all;
use sn_transfers::{is_genesis_spend, SignedSpend, SpendAddress, TransferError};
use std::{collections::BTreeSet, iter::Iterator};

impl Network {
    /// This function verifies a single spend.
    /// This is used by nodes validation to before they store it.
    /// - It does NOT check if the spend exists online
    /// - It does NOT check if the spend is already spent on the Network
    /// - It checks if the spend has valid ancestry, that its parents exist on the Network
    /// - It checks that the spend has a valid signature and content
    pub async fn verify_spend(&self, spend: &SignedSpend) -> Result<()> {
        let unique_key = spend.unique_pubkey();
        debug!("Verifying spend {unique_key}");
        spend.verify(spend.spent_tx_hash())?;

        // genesis does not have parents so we end here
        if is_genesis_spend(spend) {
            debug!("Verified {unique_key} was Genesis spend!");
            return Ok(());
        }

        // get its parents
        let parent_keys = spend
            .spend
            .parent_tx
            .inputs
            .iter()
            .map(|input| input.unique_pubkey);
        let tasks: Vec<_> = parent_keys
            .map(|a| self.get_spend(SpendAddress::from_unique_pubkey(&a)))
            .collect();
        let parent_spends: BTreeSet<SignedSpend> = join_all(tasks)
            .await
            .into_iter()
            .collect::<Result<BTreeSet<_>>>()
            .map_err(|e| {
                let s = format!("Failed to get parent spend: {e}");
                warn!("{}", s);
                Error::Transfer(TransferError::InvalidParentSpend(s))
            })?;

        // verify the parents
        spend.verify_parent_spends(parent_spends.iter())?;

        Ok(())
    }
}
