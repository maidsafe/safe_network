// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use itertools::Itertools;
use sn_dbc::{DbcId, DbcTransaction, TransactionVerifier};
use sn_networking::Network;
use sn_protocol::{
    error::{Error, Result},
    messages::{Query, QueryResponse, Request, Response},
    storage::{DbcAddress, SpendWithParent},
};
use sn_transfers::dbc_genesis::{is_genesis_parent_tx, GENESIS_DBC};
use std::{
    collections::{BTreeSet, HashSet},
    iter::Iterator,
};

/// Aggregates all the spends for the provided DbcId from the network and aggregates them.
/// It performs Spend aggregation and returns a Vec with a max length of 2
/// - No spend exists if len() == 0
/// - Valid spend if len() == 1
/// - Double spend if len() > 1
pub(crate) async fn get_aggregated_spends_from_peers(
    network: &Network,
    dbc_id: DbcId,
) -> Result<Vec<SpendWithParent>> {
    let address = DbcAddress::from_dbc_id(&dbc_id);
    let request = Request::Query(Query::GetSpend(address));
    debug!("Trying to GetSepnd from the closest nodes to {:?}", address);
    let responses = network.node_send_to_closest(&request).await.map_err(|e| {
        warn!("Error while fetching spends on the Network for {address:?}: {e:?}");
        Error::SpendNotFound(address)
    })?;

    // Get all the distinct spends
    let mut spends = HashSet::new();
    responses.into_iter().flatten().for_each(|resp| {
        match resp {
            Response::Query(QueryResponse::GetDbcSpend(Ok(signed_spend))) => {
                let _ = spends.insert(signed_spend);
            }
            Response::Query(QueryResponse::GetDbcSpend(Err(Error::DoubleSpendAttempt(
                spend_one,
                spend_two,
            )))) => {
                warn!("Double spend attempt reported by peer: {spend_one:?} and {spend_two:?}, collecting them");
                let _ = spends.insert(*spend_one);
                let _ = spends.insert(*spend_two);
            }
            Response::Query(QueryResponse::GetDbcSpend(Err(e))) => {
                warn!("Peer sent us an error while getting spend from network: {e:?}");
            }
            _ => {
                // TODO check what it means if we get a different response type
            }
        }
    });
    Ok(aggregate_spends(spends, dbc_id))
}

/// Aggregates the provided set of spends by,
/// - Making sure the Spends are unique
/// - Making sure the DbcId match the provided one
/// - Verifying the `spent_tx_hash`
/// - Sorting and returning < 2 spends as output
pub(crate) fn aggregate_spends<I>(spends: I, valid_dbc_id: DbcId) -> Vec<SpendWithParent>
where
    I: IntoIterator<Item = SpendWithParent>,
{
    let spends = spends.into_iter().collect::<HashSet<_>>();
    // on the unique set of SignedSpends, perform the below filter + sort
    spends
        .into_iter()
        // make sure the dbc_id and the signature are valid.
        .filter(|spend_with_parent| {
            // make sure the dbc_ids are the same
            spend_with_parent.signed_spend.dbc_id() == &valid_dbc_id
                // make sure the spent_tx hash matches
                && spend_with_parent
                    .signed_spend
                    .verify(spend_with_parent.signed_spend.spent_tx_hash())
                    .is_ok()
                // make sure the parent_tx hash matches
                && spend_with_parent.parent_tx.hash()
                    == spend_with_parent.signed_spend.dbc_creation_tx_hash()
        })
        // must be ordered to just store 2 of them.
        .sorted_by(|a, b| a.cmp(b))
        .take(2)
        .collect()
}

/// Fetch all parent spends from the network and check them
/// they should all exist as valid spends for this current spend attempt to be valid
/// The signed_spend.dbc_id() shall exist among the parent_tx's outputs.
pub(crate) async fn check_parent_spends(
    network: &Network,
    spend_with_parent: &SpendWithParent,
) -> Result<()> {
    // skip check if the spent DBC is Genesis
    if is_genesis_parent_tx(&spend_with_parent.parent_tx)
        && spend_with_parent.signed_spend.dbc_id() == &GENESIS_DBC.id
    {
        trace!(
            "Validated parent_spends because spent DBC is Genesis: {:?}",
            spend_with_parent.signed_spend.dbc_id()
        );
        return Ok(());
    }

    // check that the spent DBC is an output of the parent tx
    if !spend_with_parent
        .parent_tx
        .outputs
        .iter()
        .any(|o| o.dbc_id() == spend_with_parent.signed_spend.dbc_id())
    {
        return Err(Error::InvalidParentTx(format!(
            "The DBC we're trying to spend: {:?} is not an output of the parent tx: {:?}",
            spend_with_parent.signed_spend, spend_with_parent.parent_tx
        )));
    }

    // get the parent spends from the network
    trace!(
        "Getting parent_spends for {:?}",
        spend_with_parent.signed_spend.dbc_id()
    );
    let parent_spends = match get_parent_spends(network, &spend_with_parent.parent_tx).await {
        Ok(parent_spends) => parent_spends,
        Err(e) => return Err(e)?,
    };

    // check the parent spends
    trace!(
        "Validating parent_spends for {:?}",
        spend_with_parent.signed_spend.dbc_id()
    );
    validate_parent_spends(spend_with_parent, parent_spends)?;

    trace!(
        "Validated parent_spends for {:?}",
        spend_with_parent.signed_spend.dbc_id()
    );
    Ok(())
}

/// The src_tx is the tx where the dbc to spend, was created.
/// The signed_spend.dbc_id() shall exist among its outputs.
fn validate_parent_spends(
    spend_with_parent: &SpendWithParent,
    parent_spends: BTreeSet<SpendWithParent>,
) -> Result<()> {
    // Check that the parent spends are all from the parent tx
    for parent_spend in &parent_spends {
        let tx_our_dbc_was_created_in = spend_with_parent.signed_spend.dbc_creation_tx_hash();
        let tx_its_parents_where_spent_in = parent_spend.signed_spend.spent_tx_hash();
        if tx_our_dbc_was_created_in != tx_its_parents_where_spent_in {
            return Err(Error::BadParentSpendHash(format!(
                "One of the parents was spent in another transaction. Expected: {tx_our_dbc_was_created_in:?} Got: {tx_its_parents_where_spent_in:?}"
            )));
        }
    }

    // Here we check that the DBC we're trying to spend was created in a valid tx
    let parent_spends = parent_spends
        .into_iter()
        .map(|spend| spend.signed_spend)
        .collect();
    if let Err(e) = TransactionVerifier::verify(&spend_with_parent.parent_tx, &parent_spends) {
        return Err(Error::InvalidParentTx(format!(
            "verification failed for parent tx for {:?}: {e:?}",
            spend_with_parent.signed_spend.dbc_id()
        )));
    }

    Ok(())
}

/// Fetch all parent spends from the network.
/// Checks for double spend on any of the parent_input
async fn get_parent_spends(
    network: &Network,
    parent_tx: &DbcTransaction,
) -> Result<BTreeSet<SpendWithParent>> {
    let mut all_parent_spends = BTreeSet::new();

    // First we fetch all parent spends from the network.
    // They shall naturally all exist as valid spends for this current
    // spend attempt to be valid.
    for parent_input in &parent_tx.inputs {
        // This call makes sure we get the same spend from all in the close group.
        // If we receive a spend here, it is assumed to be valid. But we will verify
        // that anyway, in the code right after this for loop.
        trace!("Getting parent_spend for {:?}", parent_input.dbc_id());
        let mut parent_spends =
            get_aggregated_spends_from_peers(network, parent_input.dbc_id()).await?;
        if parent_spends.len() > 1 {
            warn!(
                "Got a double spend for the parent input {:?}",
                parent_input.dbc_id()
            );
            let mut proof = parent_spends.iter().take(2);
            if let (Some(spend_one), Some(spend_two)) = (proof.next(), proof.next()) {
                return Err(Error::DoubleSpendAttempt(
                    Box::new(spend_one.to_owned()),
                    Box::new(spend_two.to_owned()),
                ))?;
            }
        } else if parent_spends.is_empty() {
            let err = Error::InsufficientValidSpendsFound(DbcAddress::from_dbc_id(
                &parent_input.dbc_id(),
            ));
            error!("Failed to get parent_spends {err:?}");
            return Err(err);
        }

        let single_parent_spend = parent_spends.remove(0);
        trace!("got parent_spend for {:?}", parent_input.dbc_id());
        let _ = all_parent_spends.insert(single_parent_spend);
    }

    Ok(all_parent_spends)
}
