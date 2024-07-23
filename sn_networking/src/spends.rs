// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{Network, NetworkError, Result};
use futures::future::join_all;
use sn_transfers::{is_genesis_spend, SignedSpend, SpendAddress, TransferError};
use std::collections::BTreeSet;

impl Network {
    /// This function verifies a single spend.
    /// This is used by nodes for spends validation, before storing them.
    /// - It checks if the spend has valid ancestry, that its parents exist on the Network.
    /// - If the parent is a double spend, we still carry out the valdiation, but at the end return the error
    /// - It checks that the spend has a valid signature and content
    /// - It does NOT check if the spend exists online
    /// - It does NOT check if the spend is already spent on the Network
    pub async fn verify_spend(&self, spend: &SignedSpend) -> Result<()> {
        let unique_key = spend.unique_pubkey();
        debug!("Verifying spend {unique_key}");
        spend.verify()?;

        // genesis does not have parents so we end here
        if is_genesis_spend(spend) {
            debug!("Verified {unique_key} was Genesis spend!");
            return Ok(());
        }

        // get its parents
        let mut result = Ok(());
        let parent_keys = spend.spend.ancestors.clone();
        let tasks: Vec<_> = parent_keys
            .iter()
            .map(|parent| async move {
                let spend = self
                    .get_spend(SpendAddress::from_unique_pubkey(parent))
                    .await;
                (*parent, spend)
            })
            .collect();
        let mut parent_spends = BTreeSet::new();
        for (parent_key, parent_spend) in join_all(tasks).await {
            match parent_spend {
                Ok(parent_spend) => {
                    parent_spends.insert(parent_spend);
                }
                Err(NetworkError::DoubleSpendAttempt(attempts)) => {
                    warn!("While verifying {unique_key:?}, a double spend attempt ({attempts:?}) detected for the parent with pub key {parent_key:?} . Continuing verification.");
                    parent_spends.extend(attempts);
                    result = Err(NetworkError::Transfer(TransferError::DoubleSpentParent));
                }
                Err(e) => {
                    let s = format!("Failed to get parent spend of {unique_key} parent pubkey: {parent_key:?} error: {e}");
                    warn!("{}", s);
                    return Err(NetworkError::Transfer(TransferError::InvalidParentSpend(s)));
                }
            }
        }

        // verify the parents
        spend.verify_parent_spends(&parent_spends)?;

        result
    }
}
