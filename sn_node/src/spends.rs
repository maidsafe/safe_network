// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use itertools::Itertools;
use sn_dbc::{DbcId, SignedSpend};
use sn_protocol::{
    error::{Error, Result},
    storage::DbcAddress,
};
use sn_transfers::dbc_genesis::{is_genesis_parent_tx, GENESIS_DBC};
use std::{
    collections::{BTreeSet, HashSet},
    iter::Iterator,
};

/// Aggregates the provided set of spends by,
/// - Making sure the Spends are unique
/// - Making sure the DbcId match the provided one
/// - Verifying the `spent_tx_hash`
/// - Sorting and returning < 2 spends as output
pub(crate) fn aggregate_spends<I>(spends: I, valid_dbc_id: DbcId) -> Vec<SignedSpend>
where
    I: IntoIterator<Item = SignedSpend>,
{
    trace!(
        "aggregating spends for {:?}",
        DbcAddress::from_dbc_id(&valid_dbc_id)
    );

    let spends = spends.into_iter().collect::<HashSet<_>>();
    // on the unique set of SignedSpends, perform the below filter + sort
    spends
        .into_iter()
        // make sure the dbc_id and the signature are valid.
        .filter(|signed_spend| {
            // make sure the dbc_ids are the same
            let is_valid_dbc_id = signed_spend.dbc_id() == &valid_dbc_id;

            // don't verify if we already failed this check
            if !is_valid_dbc_id {
                trace!("Aggregating spend, is not valid dbc id, this dbc {:?}, expected {:?}", signed_spend.dbc_id(), &valid_dbc_id);

                return false
            }

            // make sure the spent_tx hash matches
            let spent_tx_hash_matches = signed_spend
            .verify(signed_spend.spent_tx_hash())
            .is_ok();
            trace!("Aggregating spend, is_valid_dbc_id: {is_valid_dbc_id}, spent_tx_hash_matches: {spent_tx_hash_matches} ");
            is_valid_dbc_id && spent_tx_hash_matches
        })
        // must be ordered to just store 2 of them.
        .sorted_by(|a, b| a.cmp(b))
        .take(2)
        .collect()
}

/// Fetch all parent spends from the network and check them
/// they should all exist as valid spends for this current spend attempt to be valid
/// The signed_spend.dbc_id() shall exist among the parent_tx's outputs.
pub(crate) fn check_parent_spends(
    parent_spends: &BTreeSet<SignedSpend>,
    signed_spend: &SignedSpend,
) -> Result<()> {
    // skip check if the spent DBC is Genesis
    if is_genesis_parent_tx(&signed_spend.spend.dbc_creation_tx)
        && signed_spend.dbc_id() == &GENESIS_DBC.id
    {
        trace!(
            "Validated parent_spends because spent DBC is Genesis: {:?}",
            signed_spend.dbc_id()
        );
        return Ok(());
    }

    // check that the spent DBC is an output of the parent tx
    if !signed_spend
        .spend
        .dbc_creation_tx
        .outputs
        .iter()
        .any(|o| o.dbc_id() == signed_spend.dbc_id())
    {
        return Err(Error::SpendParentTxInvalid(format!(
            "The DBC we're trying to spend: {:?} is not an output of the parent tx: {:?}",
            signed_spend, signed_spend.spend.dbc_creation_tx
        )));
    }

    // check the parent spends
    trace!("Validating parent_spends for {:?}", signed_spend.dbc_id());
    validate_parent_spends(signed_spend, parent_spends)?;

    trace!("Validated parent_spends for {:?}", signed_spend.dbc_id());
    Ok(())
}

/// The src_tx is the tx where the dbc to spend, was created.
/// The signed_spend.dbc_id() shall exist among its outputs.
fn validate_parent_spends(
    signed_spend: &SignedSpend,
    parent_spends: &BTreeSet<SignedSpend>,
) -> Result<()> {
    // Check that the parent spends are all from the parent tx
    for parent_spend in parent_spends {
        let tx_our_dbc_was_created_in = signed_spend.dbc_creation_tx_hash();
        let tx_its_parents_where_spent_in = parent_spend.spent_tx_hash();
        if tx_our_dbc_was_created_in != tx_its_parents_where_spent_in {
            return Err(Error::SpendParentTxInvalid(format!(
                "One of the parents was spent in another transaction. Expected: {tx_our_dbc_was_created_in:?} Got: {tx_its_parents_where_spent_in:?}"
            )));
        }
    }

    // Here we check that the DBC we're trying to spend was created in a valid tx
    if let Err(e) = signed_spend
        .spend
        .dbc_creation_tx
        .verify_against_inputs_spent(parent_spends)
    {
        return Err(Error::SpendParentTxInvalid(format!(
            "verification failed for parent tx for {:?}: {e:?}",
            signed_spend.dbc_id()
        )));
    }

    Ok(())
}
