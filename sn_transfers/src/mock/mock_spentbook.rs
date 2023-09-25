// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::GenesisMaterial;
use crate::{
    transaction::Transaction, unique_keys::MainPubkey, Error, Hash, NanoTokens, Output, Result,
    SignedSpend, UniquePubkey,
};
use std::collections::{BTreeMap, HashMap};

/// This is a mock SpentBook used for our test cases. A proper implementation
/// will be distributed, persistent, and auditable.
///
/// This impl attempts to be reasonably efficient.  In particular
/// it stores only a single copy of each Tx and includes indexes:
///     tx_hash    --> Tx
///     public_key  --> tx_hash
///     public_key --> Output
///
/// The public_key map eliminates a full table scan when matching
/// public keys for each input of logged Tx to public key of Output in
/// already-spent Txs.
///
/// This impl does duplicate the Outputs in the public_key index, which
/// is not ideal and should not be done for a "real" system.
///
/// Another approach would be to map public_key --> tx_hash. This eliminates
/// the need to store duplicate Output. One could lookup the Tx with
/// the desired Output, and then iterate through outputs to actually find it.
///
/// See the very first commit of this file For a naive impl that uses only
/// a single map<public_key, tx>.
#[derive(Debug, Clone)]
pub struct SpentbookNode {
    pub id: MainPubkey,
    pub transactions: HashMap<Hash, Transaction>,
    pub unique_pubkeys: BTreeMap<UniquePubkey, Hash>,
    pub outputs_by_input_id: BTreeMap<UniquePubkey, Output>,
    pub genesis: (UniquePubkey, NanoTokens),
}

impl Default for SpentbookNode {
    fn default() -> Self {
        let genesis_material = GenesisMaterial::default();
        let token = genesis_material.genesis_tx.0.inputs[0].amount;

        Self {
            id: MainPubkey::new(bls::SecretKey::random().public_key()),
            transactions: Default::default(),
            unique_pubkeys: Default::default(),
            outputs_by_input_id: Default::default(),
            genesis: (genesis_material.input_unique_pubkey, token),
        }
    }
}

impl SpentbookNode {
    pub fn iter(&self) -> impl Iterator<Item = (&UniquePubkey, &Transaction)> + '_ {
        self.unique_pubkeys.iter().map(move |(k, h)| {
            (
                k,
                match self.transactions.get(h) {
                    Some(tx) => tx,
                    // todo: something better.
                    None => panic!("Spentbook is in an inconsistent state"),
                },
            )
        })
    }

    pub fn is_spent(&self, unique_pubkey: &UniquePubkey) -> bool {
        self.unique_pubkeys.contains_key(unique_pubkey)
    }

    pub fn log_spent(&mut self, tx: &Transaction, signed_spend: &SignedSpend) -> Result<()> {
        self.log_spent_worker(tx, signed_spend, true)
    }

    // This is invalid behavior, however we provide this method for test cases
    // that need to write an invalid Tx to spentbook in order to test reissue
    // behavior.
    #[cfg(test)]
    pub fn log_spent_and_skip_tx_verification(
        &mut self,
        spent_tx: &Transaction,
        signed_spend: &SignedSpend,
    ) -> Result<()> {
        self.log_spent_worker(spent_tx, signed_spend, false)
    }

    fn log_spent_worker(
        &mut self,
        spent_tx: &Transaction,
        signed_spend: &SignedSpend,
        verify_tx: bool,
    ) -> Result<()> {
        let input_id = signed_spend.unique_pubkey();
        let spent_tx_hash = signed_spend.spent_tx_hash();
        let tx_hash = spent_tx.hash();

        if tx_hash != spent_tx_hash {
            return Err(Error::InvalidTransactionHash);
        }

        if verify_tx {
            // Do not permit invalid tx to be logged.
            spent_tx.verify()?;
        }

        // Add unique_pubkey:tx_hash to unique_pubkey index.
        let existing_tx_hash = self
            .unique_pubkeys
            .entry(*input_id)
            .or_insert_with(|| tx_hash);

        if *existing_tx_hash == tx_hash {
            // Add tx_hash:tx to transaction entries. (primary data store)
            let existing_tx = self
                .transactions
                .entry(tx_hash)
                .or_insert_with(|| spent_tx.clone());

            // Add unique_pubkey:output to unique_pubkey index.
            for output in existing_tx.outputs.iter() {
                let output_id = *output.unique_pubkey();
                // This will make later inputs able to find its previous
                // entry as an output, i.e. from when it was created.
                self.outputs_by_input_id
                    .entry(output_id)
                    .or_insert_with(|| output.clone());
            }

            Ok(())
        } else {
            Err(crate::mock::Error::CashNoteAlreadySpent.into())
        }
    }
}
