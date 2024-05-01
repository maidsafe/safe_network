// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{Network, NetworkError, Result};
use futures::future::join_all;
use sn_transfers::{
    is_genesis_spend, SignedSpend, SpendAddress, TransferError, CASHNOTE_PURPOSE_OF_CHANGE,
    CASHNOTE_PURPOSE_OF_NETWORK_ROYALTIES,
};
use std::{collections::BTreeSet, iter::Iterator};

impl Network {
    /// This function verifies a single spend.
    /// This is used by nodes for spends validation, before storing them.
    /// - It checks if the spend has valid ancestry, that its parents exist on the Network
    /// - It checks that the spend has a valid signature and content
    /// - It does NOT check if the spend exists online
    /// - It does NOT check if the spend is already spent on the Network
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
                let s = format!("Failed to get parent spend of {unique_key:?}: {e}");
                warn!("{}", s);
                NetworkError::Transfer(TransferError::InvalidParentSpend(s))
            })?;

        // verify the parents
        spend.verify_parent_spends(parent_spends.iter())?;

        Ok(())
    }

    /// This function verifies a received notification of an owner's claim of storage payment
    /// It fetches the correspondent spend, then carry out verification.
    /// Once verified, the owner and amount will be reported to Node for further record.
    pub async fn handle_storage_payment_notification(
        &self,
        spend_address: SpendAddress,
        owner: String,
        royalty: u64,
        store_cost: u64,
    ) -> Option<(String, u64, u64)> {
        let spend = match self.get_spend(spend_address).await {
            Ok(spend) => spend,
            Err(err) => {
                error!(
                    "When verify storage payment notification, cannot get spend {spend_address:?} {err:?}"
                );
                return None;
            }
        };

        let royalty_keyword = CASHNOTE_PURPOSE_OF_NETWORK_ROYALTIES.to_string();
        let change_keyword = CASHNOTE_PURPOSE_OF_CHANGE.to_string();

        // 1, The spend's outputs shall have equal number of royalty and store_cost payments
        // 2, The claimed payment shall be within the spend's outputs
        let num_of_royalties = spend
            .spent_tx()
            .outputs
            .iter()
            .filter(|o| o.purpose == royalty_keyword)
            .count();
        let num_of_change = spend
            .spent_tx()
            .outputs
            .iter()
            .filter(|o| o.purpose == change_keyword)
            .count();
        let num_of_store_cost = spend.spent_tx().outputs.len() - num_of_royalties - num_of_change;

        let payments_match = num_of_store_cost == num_of_royalties;

        let find_royalty = spend
            .spent_tx()
            .outputs
            .iter()
            .any(|o| o.purpose == royalty_keyword && o.amount.as_nano() == royalty);
        let find_store_cost = spend
            .spent_tx()
            .outputs
            .iter()
            .any(|o| o.purpose == owner && o.amount.as_nano() == store_cost);

        if payments_match && find_royalty && find_store_cost {
            Some((owner, royalty, store_cost))
        } else {
            error!("Claimed storage payment of ({owner} {royalty} {store_cost}) cann't be verified by the spend {spend:?}");
            None
        }
    }
}
