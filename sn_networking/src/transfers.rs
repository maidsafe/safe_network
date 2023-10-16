// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::collections::BTreeSet;

use crate::{GetQuorum, Network};

use sn_protocol::{
    error::{Error, Result},
    storage::{try_deserialize_record, RecordHeader, RecordKind, SpendAddress},
    NetworkAddress, PrettyPrintRecordKey,
};
use sn_transfers::{CashNote, DerivationIndex, SignedSpend, Transaction, UniquePubkey};
use sn_transfers::{CashNoteRedemption, LocalWallet, MainPubkey, Transfer};
use tokio::task::JoinSet;

impl Network {
    /// Gets a spend from the Network.
    pub async fn get_spend(&self, address: SpendAddress, re_attempt: bool) -> Result<SignedSpend> {
        let key = NetworkAddress::from_cash_note_address(address).to_record_key();
        let record = self
            .get_record_from_network(key, None, GetQuorum::All, re_attempt, Default::default())
            .await
            .map_err(|_| Error::SpendNotFound(address))?;
        debug!(
            "Got record from the network, {:?}",
            PrettyPrintRecordKey::from(&record.key)
        );
        let header =
            RecordHeader::from_record(&record).map_err(|_| Error::SpendNotFound(address))?;

        if let RecordKind::Spend = header.kind {
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
            Err(Error::RecordKindMismatch(RecordKind::Spend))
        }
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
        wallet: &LocalWallet,
    ) -> Result<Vec<CashNote>> {
        // get CashNoteRedemptions from encrypted Transfer
        trace!("Decyphering Transfer");
        let cashnote_redemptions = wallet
            .unwrap_transfer(transfer)
            .map_err(|_| Error::FailedToDecypherTransfer)?;

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
        trace!("Getting parent Tx for validation");
        let parent_addrs: BTreeSet<SpendAddress> = cashnote_redemptions
            .iter()
            .map(|u| u.parent_spend)
            .collect();
        let mut tasks = JoinSet::new();
        for addr in parent_addrs.clone() {
            let self_clone = self.clone();
            let _ = tasks.spawn(async move { self_clone.get_spend(addr, false).await });
        }
        let mut parent_spends = BTreeSet::new();
        while let Some(result) = tasks.join_next().await {
            let signed_spend = result
                .map_err(|_| Error::FailedToGetTransferParentSpend)?
                .map_err(|e| Error::InvalidTransfer(format!("{e}")))?;
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
                .ok_or(Error::InvalidTransfer(
                    "None of the CashNoteRedemptions are refered to in upstream Txs".to_string(),
                ))?
                .clone();
            let signed_spends: BTreeSet<SignedSpend> = parent_spends
                .iter()
                .filter(|s| s.spent_tx_hash() == src_tx.hash())
                .cloned()
                .collect();
            let cash_note = CashNote {
                id,
                src_tx,
                signed_spends,
                main_pubkey,
                derivation_index,
            };
            our_output_cash_notes.push(cash_note);
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

        Ok(our_output_cash_notes)
    }
}
