// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    close_group_majority, driver::GetRecordCfg, GetRecordError, Network, NetworkError, Result,
};
use libp2p::kad::{Quorum, Record};
use sn_protocol::{
    storage::{try_deserialize_record, RecordHeader, RecordKind, RetryStrategy, SpendAddress},
    NetworkAddress, PrettyPrintRecordKey,
};
use sn_transfers::{
    CashNote, CashNoteRedemption, DerivationIndex, HotWallet, MainPubkey, SignedSpend, Transaction,
    Transfer, UniquePubkey,
};
use std::collections::BTreeSet;
use tokio::task::JoinSet;

impl Network {
    /// Gets raw spends from the Network.
    /// For normal use please prefer using `get_spend` instead.
    /// Double spends returned together as is, not as an error.
    /// The target may have high chance not present in the network yet.
    ///
    /// If we get a quorum error, we enable re-try
    pub async fn get_raw_spends(&self, address: SpendAddress) -> Result<Vec<SignedSpend>> {
        let key = NetworkAddress::from_spend_address(address).to_record_key();
        let get_cfg = GetRecordCfg {
            get_quorum: Quorum::Majority,
            retry_strategy: None,
            // This should not be set here. This function is used as a quick check to find the spends around the key during
            // validation. The returned records might possibly be double spend attempt and the record will not match
            // what we will have in hand.
            target_record: None,
            expected_holders: Default::default(),
        };
        let record = self.get_record_from_network(key.clone(), &get_cfg).await?;
        debug!(
            "Got record from the network, {:?}",
            PrettyPrintRecordKey::from(&record.key)
        );
        get_raw_signed_spends_from_record(&record)
    }

    /// Gets a spend from the Network.
    /// We know it must be there, and has to be fetched from Quorum::All
    ///
    /// If we get a quorum error, we increase the RetryStrategy
    pub async fn get_spend(&self, address: SpendAddress) -> Result<SignedSpend> {
        let key = NetworkAddress::from_spend_address(address).to_record_key();
        let mut get_cfg = GetRecordCfg {
            get_quorum: Quorum::All,
            retry_strategy: Some(RetryStrategy::Quick),
            target_record: None,
            expected_holders: Default::default(),
        };
        let record = match self.get_record_from_network(key.clone(), &get_cfg).await {
            Ok(record) => record,
            Err(NetworkError::GetRecordError(GetRecordError::NotEnoughCopies {
                record,
                expected,
                got,
            })) => {
                // if majority holds the spend, it might be worth to be trusted.
                if got >= close_group_majority() {
                    debug!("At least a majority nodes hold the spend {address:?}, going to trust it if can fetch with majority again.");
                    get_cfg.get_quorum = Quorum::Majority;
                    get_cfg.retry_strategy = Some(RetryStrategy::Balanced);
                    self.get_record_from_network(key, &get_cfg).await?
                } else {
                    return Err(NetworkError::GetRecordError(
                        GetRecordError::NotEnoughCopies {
                            record,
                            expected,
                            got,
                        },
                    ));
                }
            }
            Err(err) => return Err(err),
        };
        debug!(
            "Got record from the network, {:?}",
            PrettyPrintRecordKey::from(&record.key)
        );

        get_signed_spend_from_record(&address, &record)
    }

    /// This function is used to receive a Transfer and turn it back into spendable CashNotes.
    /// Needs Network connection.
    /// Verify Transfer and rebuild spendable currency from it
    /// Returns an `Error::FailedToDecypherTransfer` if the transfer cannot be decyphered
    /// (This means the transfer is not for us as it was not encrypted to our key)
    /// Returns an `Error::InvalidTransfer` if the transfer is not valid
    /// Else returns a list of CashNotes that can be deposited to our wallet and spent
    pub async fn verify_and_unpack_transfer(
        &self,
        transfer: &Transfer,
        wallet: &HotWallet,
    ) -> Result<Vec<CashNote>> {
        // get CashNoteRedemptions from encrypted Transfer
        debug!("Decyphering Transfer");
        let cashnote_redemptions = wallet.unwrap_transfer(transfer)?;

        self.verify_cash_notes_redemptions(wallet.address(), &cashnote_redemptions)
            .await
    }

    /// This function is used to receive a list of CashNoteRedemptions and turn it back into spendable CashNotes.
    /// Needs Network connection.
    /// Verify CashNoteRedemptions and rebuild spendable currency from them.
    /// Returns an `Error::InvalidTransfer` if any CashNoteRedemption is not valid
    /// Else returns a list of CashNotes that can be spent by the owner.
    pub async fn verify_cash_notes_redemptions(
        &self,
        main_pubkey: MainPubkey,
        cashnote_redemptions: &[CashNoteRedemption],
    ) -> Result<Vec<CashNote>> {
        // get the parent transactions
        debug!(
            "Getting parent Tx for validation from {:?}",
            cashnote_redemptions.len()
        );
        let parent_addrs: BTreeSet<SpendAddress> = cashnote_redemptions
            .iter()
            .map(|u| u.parent_spend)
            .collect();
        let mut tasks = JoinSet::new();
        for addr in parent_addrs.clone() {
            let self_clone = self.clone();
            let _ = tasks.spawn(async move { self_clone.get_spend(addr).await });
        }
        let mut parent_spends = BTreeSet::new();
        while let Some(result) = tasks.join_next().await {
            let signed_spend = result
                .map_err(|e| NetworkError::FailedToGetSpend(format!("{e}")))?
                .map_err(|e| NetworkError::InvalidTransfer(format!("{e}")))?;
            let _ = parent_spends.insert(signed_spend.clone());
        }
        let parent_txs: BTreeSet<Transaction> =
            parent_spends.iter().map(|s| s.spent_tx()).collect();

        // get our outputs from Tx
        let our_output_unique_pubkeys: Vec<(UniquePubkey, DerivationIndex)> = cashnote_redemptions
            .iter()
            .map(|u| {
                let unique_pubkey = main_pubkey.new_unique_pubkey(&u.derivation_index);
                (unique_pubkey, u.derivation_index)
            })
            .collect();
        let mut our_output_cash_notes = Vec::new();

        for (id, derivation_index) in our_output_unique_pubkeys.into_iter() {
            let src_tx = parent_txs
                .iter()
                .find(|tx| tx.outputs.iter().any(|o| o.unique_pubkey() == &id))
                .ok_or(NetworkError::InvalidTransfer(
                    "None of the CashNoteRedemptions are destined to our key".to_string(),
                ))?
                .clone();
            let signed_spends: BTreeSet<SignedSpend> = parent_spends
                .iter()
                .filter(|s| s.spent_tx_hash() == src_tx.hash())
                .cloned()
                .collect();
            let cash_note = CashNote {
                unique_pubkey: id,
                parent_tx: src_tx,
                parent_spends: signed_spends,
                main_pubkey,
                derivation_index,
            };
            our_output_cash_notes.push(cash_note);
        }

        // check Txs and parent spends are valid
        debug!("Validating parent spends");
        for tx in parent_txs {
            let tx_inputs_keys: Vec<_> = tx.inputs.iter().map(|i| i.unique_pubkey()).collect();

            // get the missing inputs spends from the network
            let mut tasks = JoinSet::new();
            for input_key in tx_inputs_keys {
                if parent_spends.iter().any(|s| s.unique_pubkey() == input_key) {
                    continue;
                }
                let self_clone = self.clone();
                let addr = SpendAddress::from_unique_pubkey(input_key);
                let _ = tasks.spawn(async move { self_clone.get_spend(addr).await });
            }
            while let Some(result) = tasks.join_next().await {
                let signed_spend = result
                    .map_err(|e| NetworkError::FailedToGetSpend(format!("{e}")))?
                    .map_err(|e| NetworkError::InvalidTransfer(format!("{e}")))?;
                let _ = parent_spends.insert(signed_spend.clone());
            }

            // verify the Tx against the inputs spends
            let input_spends: BTreeSet<_> = parent_spends
                .iter()
                .filter(|s| s.spent_tx_hash() == tx.hash())
                .cloned()
                .collect();
            tx.verify_against_inputs_spent(&input_spends).map_err(|e| {
                NetworkError::InvalidTransfer(format!(
                    "Payment parent Tx {:?} invalid: {e}",
                    tx.hash()
                ))
            })?;
        }

        Ok(our_output_cash_notes)
    }
}

/// Tries to get the signed spend out of a record as is, double spends are returned together as is.
pub fn get_raw_signed_spends_from_record(record: &Record) -> Result<Vec<SignedSpend>> {
    let header = RecordHeader::from_record(record)?;
    if let RecordKind::Spend = header.kind {
        let spends = try_deserialize_record::<Vec<SignedSpend>>(record)?;
        Ok(spends)
    } else {
        warn!(
            "RecordKind mismatch while trying to retrieve spends from record {:?}",
            PrettyPrintRecordKey::from(&record.key)
        );
        Err(NetworkError::RecordKindMismatch(RecordKind::Spend))
    }
}

/// Get the signed spend out of a record.
/// Double spends are returned as an error
pub fn get_signed_spend_from_record(
    address: &SpendAddress,
    record: &Record,
) -> Result<SignedSpend> {
    let spends = get_raw_signed_spends_from_record(record)?;
    match spends.as_slice() {
        [] => {
            error!("Found no spend for {address:?}");
            Err(NetworkError::NoSpendFoundInsideRecord(*address))
        }
        [one] => {
            debug!("Spend get for address: {address:?} successful");
            Ok(one.clone())
        }
        _double_spends => {
            warn!(
                "Found double spend(s) of len {} for {address:?}",
                spends.len()
            );
            Err(NetworkError::DoubleSpendAttempt(spends))
        }
    }
}
