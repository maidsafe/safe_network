// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{Error, Result};
use itertools::Itertools;
use sn_protocol::storage::SpendAddress;
use sn_transfers::{is_genesis_parent_tx, SignedSpend, UniquePubkey, GENESIS_CASHNOTE};
use std::{
    collections::{BTreeSet, HashSet},
    iter::Iterator,
};

/// Aggregates the provided set of spends by,
/// - Making sure the Spends are unique
/// - Making sure the UniquePubkey match the provided one
/// - Verifying the `spent_tx_hash`
/// - Sorting and returning < 2 spends as output
pub(crate) fn aggregate_spends<I>(spends: I, valid_unique_pubkey: UniquePubkey) -> Vec<SignedSpend>
where
    I: IntoIterator<Item = SignedSpend>,
{
    trace!(
        "aggregating spends for {:?}",
        SpendAddress::from_unique_pubkey(&valid_unique_pubkey)
    );

    let spends = spends.into_iter().collect::<HashSet<_>>();
    // on the unique set of SignedSpends, perform the below filter + sort
    spends
        .into_iter()
        // make sure the unique_pubkey and the signature are valid.
        .filter(|signed_spend| {
            // make sure the unique_pubkeys are the same
            let is_valid_unique_pubkey = signed_spend.unique_pubkey() == &valid_unique_pubkey;

            // don't verify if we already failed this check
            if !is_valid_unique_pubkey {
                trace!("Aggregating spend, is not valid cash_note id, this cash_note {:?}, expected {:?}", signed_spend.unique_pubkey(), &valid_unique_pubkey);

                return false
            }

            // make sure the spent_tx hash matches
            let spent_tx_hash_matches = signed_spend
            .verify(signed_spend.spent_tx_hash())
            .is_ok();
            trace!("Aggregating spend, is_valid_unique_pubkey: {is_valid_unique_pubkey}, spent_tx_hash_matches: {spent_tx_hash_matches} ");
            is_valid_unique_pubkey && spent_tx_hash_matches
        })
        // must be ordered to just store 2 of them.
        .sorted_by(|a, b| a.cmp(b))
        .take(2)
        .collect()
}

/// Fetch all parent spends from the network and check them
/// they should all exist as valid spends for this current spend attempt to be valid
/// The signed_spend.unique_pubkey() shall exist among the parent_tx's outputs.
pub(crate) fn check_parent_spends(
    parent_spends: &BTreeSet<SignedSpend>,
    signed_spend: &SignedSpend,
) -> Result<()> {
    // skip check if the spent CashNote is Genesis
    if is_genesis_parent_tx(&signed_spend.spend.parent_tx)
        && signed_spend.unique_pubkey() == &GENESIS_CASHNOTE.id
    {
        trace!(
            "Validated parent_spends because spent CashNote is Genesis: {:?}",
            signed_spend.unique_pubkey()
        );
        return Ok(());
    }

    // check that the spent CashNote is an output of the parent tx
    if !signed_spend
        .spend
        .parent_tx
        .outputs
        .iter()
        .any(|o| o.unique_pubkey() == signed_spend.unique_pubkey())
    {
        return Err(Error::SpendParentTxInvalid(format!(
            "The CashNote we're trying to spend: {:?} is not an output of the parent tx: {:?}",
            signed_spend, signed_spend.spend.parent_tx
        )));
    }

    // check the parent spends
    trace!(
        "Validating parent_spends for {:?}",
        signed_spend.unique_pubkey()
    );
    validate_parent_spends(signed_spend, parent_spends)?;

    trace!(
        "Validated parent_spends for {:?}",
        signed_spend.unique_pubkey()
    );
    Ok(())
}

/// The src_tx is the tx where the cash_note to spend, was created.
/// The signed_spend.unique_pubkey() shall exist among its outputs.
fn validate_parent_spends(
    signed_spend: &SignedSpend,
    parent_spends: &BTreeSet<SignedSpend>,
) -> Result<()> {
    // Check that the parent spends are all from the parent tx
    for parent_spend in parent_spends {
        let tx_our_cash_note_was_created_in = signed_spend.parent_tx_hash();
        let tx_its_parents_where_spent_in = parent_spend.spent_tx_hash();
        if tx_our_cash_note_was_created_in != tx_its_parents_where_spent_in {
            return Err(Error::SpendParentTxInvalid(format!(
                "One of the parents was spent in another transaction. Expected: {tx_our_cash_note_was_created_in:?} Got: {tx_its_parents_where_spent_in:?}"
            )));
        }
    }

    // Here we check that the CashNote we're trying to spend was created in a valid tx
    if let Err(e) = signed_spend
        .spend
        .parent_tx
        .verify_against_inputs_spent(parent_spends)
    {
        return Err(Error::SpendParentTxInvalid(format!(
            "verification failed for parent tx for {:?}: {e:?}",
            signed_spend.unique_pubkey()
        )));
    }

    Ok(())
}
