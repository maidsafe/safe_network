// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::collections::BTreeSet;

use crate::Network;

use sn_dbc::{Dbc, DbcId, DbcSecrets, DbcTransaction, DerivationIndex, SignedSpend};
use sn_protocol::{
    error::{Error, Result},
    storage::{try_deserialize_record, DbcAddress, RecordHeader, RecordKind},
    NetworkAddress, PrettyPrintRecordKey,
};
use sn_transfers::wallet::{LocalWallet, Transfer};
use tokio::task::JoinSet;

impl Network {
    /// Gets a spend from the Network.
    pub async fn get_spend(&self, address: DbcAddress, re_attempt: bool) -> Result<SignedSpend> {
        let key = NetworkAddress::from_dbc_address(address).to_record_key();
        let record = self
            .get_record_from_network(key, None, re_attempt)
            .await
            .map_err(|_| Error::SpendNotFound(address))?;
        debug!(
            "Got record from the network, {:?}",
            PrettyPrintRecordKey::from(record.key.clone())
        );
        let header =
            RecordHeader::from_record(&record).map_err(|_| Error::SpendNotFound(address))?;

        if let RecordKind::DbcSpend = header.kind {
            match try_deserialize_record::<Vec<SignedSpend>>(&record)
                .map_err(|_| Error::SpendNotFound(address))?
                .as_slice()
            {
                [one, two, ..] => {
                    error!("Found double spend for {address:?}");
                    Err(Error::DoubleSpendAttempt(
                        Box::new(one.to_owned()),
                        Box::new(two.to_owned()),
                    ))
                }
                [one] => {
                    trace!("Spend get for address: {address:?} successful");
                    Ok(one.clone())
                }
                _ => {
                    trace!("Found no spend for {address:?}");
                    Err(Error::SpendNotFound(address))
                }
            }
        } else {
            error!("RecordKind mismatch while trying to retrieve a Vec<SignedSpend>");
            Err(Error::RecordKindMismatch(RecordKind::DbcSpend))
        }
    }

    /// This function is used to receive a Transfer and turn it back into spendable DBCs.
    /// Needs Network connection.
    /// Verify Transfer and rebuild spendable currency from it
    /// Returns an `Error::FailedToDecypherTransfer` if the transfer cannot be decyphered
    /// (This means the transfer is not for us as it was not encrypted to our key)
    /// Returns an `Error::InvalidTransfer` if the transfer is not valid
    /// Else returns a list of DBCs that can be deposited to our wallet and spent
    pub async fn verify_and_unpack_transfer(
        &self,
        transfer: Transfer,
        wallet: &LocalWallet,
    ) -> Result<Vec<Dbc>> {
        // get UTXOs from encrypted Transfer
        trace!("Decyphering Transfer");
        let utxos = wallet
            .unwrap_transfer(transfer)
            .map_err(|_| Error::FailedToDecypherTransfer)?;
        let public_address = wallet.address();

        // get the parent transactions
        trace!("Getting parent Tx for validation");
        let parent_addrs: BTreeSet<DbcAddress> = utxos.iter().map(|u| u.parent_spend).collect();
        let mut tasks = JoinSet::new();
        for addr in parent_addrs.clone() {
            let self_clone = self.clone();
            let _ = tasks.spawn(async move { self_clone.get_spend(addr, true).await });
        }
        let mut parent_spends = BTreeSet::new();
        while let Some(result) = tasks.join_next().await {
            let signed_spend = result
                .map_err(|_| Error::FailedToGetTransferParentSpend)?
                .map_err(|e| Error::InvalidTransfer(format!("{e}")))?;
            let _ = parent_spends.insert(signed_spend.clone());
        }
        let parent_txs: BTreeSet<DbcTransaction> =
            parent_spends.iter().map(|s| s.spent_tx()).collect();

        // get all the other parent_spends from those Txs
        trace!("Getting parent spends for validation");
        let already_collected_parents = &parent_addrs;
        let other_parent_dbc_addr: BTreeSet<DbcAddress> = parent_txs
            .clone()
            .into_iter()
            .flat_map(|tx| tx.inputs)
            .map(|i| DbcAddress::from_dbc_id(&i.dbc_id()))
            .filter(|addr| !already_collected_parents.contains(addr))
            .collect();
        let mut tasks = JoinSet::new();
        for addr in other_parent_dbc_addr {
            let self_clone = self.clone();
            let _ = tasks.spawn(async move { self_clone.get_spend(addr, true).await });
        }
        while let Some(result) = tasks.join_next().await {
            let signed_spend = result
                .map_err(|_| Error::FailedToGetTransferParentSpend)?
                .map_err(|e| Error::InvalidTransfer(format!("{e}")))?;
            let _ = parent_spends.insert(signed_spend.clone());
        }

        // get our outputs from Tx
        let our_output_dbc_ids: Vec<(DbcId, DerivationIndex)> = utxos
            .iter()
            .map(|u| (wallet.derive_key(&u.derivation_index), u.derivation_index))
            .map(|(k, d)| (k.dbc_id(), d))
            .collect();
        let mut our_output_dbcs = Vec::new();
        for (id, derivation_index) in our_output_dbc_ids.into_iter() {
            let secrets = DbcSecrets {
                public_address,
                derivation_index,
            };
            let src_tx = parent_txs
                .iter()
                .find(|tx| tx.outputs.iter().any(|o| o.dbc_id() == &id))
                .ok_or(Error::InvalidTransfer(
                    "None of the UTXOs are refered to in upstream Txs".to_string(),
                ))?
                .clone();
            let signed_spends: BTreeSet<SignedSpend> = parent_spends
                .iter()
                .filter(|s| s.spent_tx_hash() == src_tx.hash())
                .cloned()
                .collect();
            let dbc = Dbc {
                id,
                src_tx,
                secrets,
                signed_spends,
            };
            our_output_dbcs.push(dbc);
        }

        // check Txs and parent spends are valid
        trace!("Validating parent spends");
        for tx in parent_txs {
            let input_spends = parent_spends
                .iter()
                .filter(|s| s.spent_tx_hash() == tx.hash())
                .cloned()
                .collect();
            tx.verify_against_inputs_spent(&input_spends)
                .map_err(|e| Error::InvalidTransfer(format!("Payment parent Tx invalid: {e}")))?;
        }

        Ok(our_output_dbcs)
    }
}
