// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use itertools::Itertools;
use sn_dbc::{DbcId, DbcTransaction, SignedSpend};
use sn_networking::Network;
use sn_protocol::{
    error::{Error, Result},
    messages::{Query, QueryResponse, Request, Response},
    storage::DbcAddress,
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
) -> Result<Vec<SignedSpend>> {
    let address = DbcAddress::from_dbc_id(&dbc_id);
    let request = Request::Query(Query::GetSpend(address));
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
pub(crate) fn aggregate_spends<I>(spends: I, valid_dbc_id: DbcId) -> Vec<SignedSpend>
where
    I: IntoIterator<Item = SignedSpend>,
{
    let spends = spends.into_iter().collect::<HashSet<_>>();
    // on the unique set of SignedSpends, perform the below filter + sort
    spends
        .into_iter()
        // make sure the dbc_id and the signature are valid.
        .filter(|spend| {
            spend.dbc_id() == &valid_dbc_id && spend.verify(spend.spent_tx_hash()).is_ok()
        })
        // must be ordered to just store 2 of them.
        .sorted_by(|a, b| a.cmp(b))
        .take(2)
        .collect()
}

/// Fetch all parent spends from the network and check them
/// they should all exist as valid spends for this current spend attempt to be valid
pub(crate) async fn check_parent_spends(
    network: &Network,
    signed_spend: &SignedSpend,
) -> Result<()> {
    trace!("Getting parent_spends for {:?}", signed_spend.dbc_id());
    let parent_spends = get_parent_spends(network, &signed_spend.spent_tx()).await?;

    trace!("Validating parent_spends for {:?}", signed_spend.dbc_id());
    validate_parent_spends(signed_spend, &signed_spend.spent_tx(), parent_spends)?;

    trace!("Validated parent_spends for {:?}", signed_spend.dbc_id());
    Ok(())
}

/// The src_tx is the tx where the dbc to spend, was created.
/// The signed_spend.dbc_id() shall exist among its outputs.
fn validate_parent_spends(
    signed_spend: &SignedSpend,
    spent_tx: &DbcTransaction,
    parent_spends: BTreeSet<SignedSpend>,
) -> Result<()> {
    // The parent_spends will be different spends,
    // one for each input that went into creating the signed_spend.
    for parent_spend in &parent_spends {
        // The dst tx of the parent must be the src tx of the spend.
        if signed_spend.dbc_creation_tx_hash() != parent_spend.spent_tx_hash() {
            return Err(Error::TxTrailMismatch {
                signed_src_tx_hash: signed_spend.dbc_creation_tx_hash(),
                parent_dst_tx_hash: parent_spend.spent_tx_hash(),
            });
        }
    }

    // We have gotten all the parent inputs from the network, so the network consider them all valid.
    // But the source tx corresponding to the signed_spend, might not match the parents' details, so that's what we check here.
    let known_parent_blinded_amounts: Vec<_> = parent_spends
        .iter()
        .map(|s| s.spend.blinded_amount)
        .collect();

    if is_genesis_parent_tx(spent_tx) && signed_spend.dbc_id() == &GENESIS_DBC.id {
        return Ok(());
    }

    // Here we check that the spend that is attempted, was created in a valid tx.
    let src_tx_validity = spent_tx.verify(&known_parent_blinded_amounts);
    if src_tx_validity.is_err() {
        return Err(Error::InvalidSourceTxProvided {
            signed_src_tx_hash: signed_spend.dbc_creation_tx_hash(),
            provided_src_tx_hash: spent_tx.hash(),
        });
    }

    Ok(())
}

/// Fetch all parent spends from the network.
/// Checks for double spend on any of the parent_input
async fn get_parent_spends(
    network: &Network,
    spent_tx: &DbcTransaction,
) -> Result<BTreeSet<SignedSpend>> {
    // These will be different spends, one for each input that went into
    // creating the above spend passed in to this function.
    let mut all_parent_spends = BTreeSet::new();

    if is_genesis_parent_tx(spent_tx) {
        trace!("Return with empty parent_spends for genesis");
        return Ok(all_parent_spends);
    }

    // First we fetch all parent spends from the network.
    // They shall naturally all exist as valid spends for this current
    // spend attempt to be valid.
    for parent_input in &spent_tx.inputs {
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
        }
        let single_parent_spend = parent_spends.remove(0);
        trace!("got parent_spend for {:?}", parent_input.dbc_id());
        let _ = all_parent_spends.insert(single_parent_spend);
    }

    Ok(all_parent_spends)
}
