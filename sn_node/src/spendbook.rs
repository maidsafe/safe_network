// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sn_networking::{close_group_majority, Network};
use sn_protocol::error::{Error, Result};
use sn_protocol::messages::{Query, QueryResponse, Request, Response};
use sn_protocol::storage::DbcAddress;
use sn_transfers::dbc_genesis::{is_genesis_parent_tx, GENESIS_DBC};

use std::collections::{BTreeMap, BTreeSet};

use libp2p::kad::{Record, RecordKey};
use sn_dbc::{DbcTransaction, SignedSpend};
use tokio::sync::RwLock;

/// The entitiy managing spends in a Node
#[derive(Default)]
pub(crate) struct SpendBook {
    /// This RW lock is here to prevent race conditions on spendbook querries
    /// that would enable double spends
    rw_lock: RwLock<()>,
}

impl SpendBook {
    /// Get a SpendBook entry for a given DbcAddress
    pub(crate) async fn spend_get(
        &self,
        network: &Network,
        address: DbcAddress,
    ) -> Result<SignedSpend> {
        trace!("Spend get for address: {address:?}");
        let _double_spend_guard = self.rw_lock.read().await;
        trace!("Handling spend get for address: {address:?}");

        // get spend from kad
        let signed_spend_bytes = match network
            .get_provided_data(RecordKey::new(address.name()))
            .await
        {
            Ok(Ok(signed_spend_bytes)) => signed_spend_bytes,
            Ok(Err(err)) | Err(err) => {
                error!("Error getting spend from local store: {err}");
                return Err(Error::SpendNotFound(address));
            }
        };

        // deserialize spend
        let signed_spend = match bincode::deserialize(&signed_spend_bytes) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to get spend because deserialization failed: {e:?}");
                return Err(Error::FailedToGetSpend(address));
            }
        };

        trace!("Spend get for address: {address:?} successful");
        Ok(signed_spend)
    }

    /// Put a SpendBook entry for a given SignedSpend
    pub(crate) async fn spend_put(
        &self,
        network: &Network,
        signed_spend: SignedSpend,
    ) -> Result<DbcAddress> {
        let dbc_id = signed_spend.dbc_id();
        let dbc_addr = DbcAddress::from_dbc_id(dbc_id);

        trace!("Spend put for {dbc_id:?} at {dbc_addr:?}");
        let _double_spend_guard = self.rw_lock.write().await;
        trace!("Handling spend put for {dbc_id:?} at {dbc_addr:?}");

        // check DBC spend
        if let Err(e) = verify_spend_dbc(network, &signed_spend).await {
            error!("Failed to store spend for {dbc_id:?} because DBC verification failed: {e:?}");
            return Err(Error::FailedToStoreSpend(dbc_addr));
        }

        // serialize spend
        let signed_spend_bytes = match bincode::serialize(&signed_spend) {
            Ok(b) => b,
            Err(e) => {
                error!("Failed to store spend for {dbc_id:?} because serialization failed: {e:?}");
                return Err(Error::FailedToStoreSpend(dbc_addr));
            }
        };

        // create a kad record and upload it
        let kademlia_record = Record {
            key: RecordKey::new(dbc_addr.name()),
            value: signed_spend_bytes,
            publisher: None,
            expires: None,
        };
        if let Err(e) = network.put_data_as_record(kademlia_record).await {
            error!("Failed to store spend {dbc_id:?}: {e:?}");
            return Err(Error::FailedToStoreSpend(dbc_addr));
        }

        trace!("Spend put for {dbc_id:?} at {dbc_addr:?} successful");
        Ok(dbc_addr)
    }

    /// Checks if two spends make up a valid double spend
    pub(crate) fn is_valid_double_spend(spend_one: &SignedSpend, spend_two: &SignedSpend) -> bool {
        spend_one != spend_two                                  // the spends are not the same one
        && spend_one.dbc_id() == spend_two.dbc_id()             // the spent DBC has the same dbc_id
        && spend_one.verify(spend_one.spent_tx_hash()).is_ok()  // the signature 1 is valid
        && spend_two.verify(spend_two.spent_tx_hash()).is_ok() // the signature 2 is valid
    }
}

/// Checks if the spend already exists in the network.
async fn check_for_double_spend(network: &Network, signed_spend: &SignedSpend) -> Result<()> {
    let dbc_addr = DbcAddress::from_dbc_id(signed_spend.dbc_id());
    let spends = match get_spend(network, dbc_addr).await {
        Ok(s) => s,
        Err(Error::DoubleSpendAttempt {
            spend_one,
            spend_two,
        }) => {
            return Err(Error::DoubleSpendAttempt {
                spend_one,
                spend_two,
            })?;
        }
        Err(e) => {
            trace!(
                "Get spend returned error while checking for double spend for {dbc_addr:?}: {e:?}"
            );
            vec![]
        }
    };

    for s in spends {
        if SpendBook::is_valid_double_spend(&s, signed_spend) {
            return Err(Error::DoubleSpendAttempt {
                spend_one: Box::new(signed_spend.clone()),
                spend_two: Box::new(s),
            })?;
        }
    }

    Ok(())
}

/// Verifies a spend to make sure it is safe to store it on the Network
/// - check if the DBC Spend is valid
/// - check if the parents of this DBC exist on the Network (recursively meaning it comes from Genesis)
/// - check if another Spend for the same DBC exists on the Network (double spend)
async fn verify_spend_dbc(network: &Network, signed_spend: &SignedSpend) -> Result<()> {
    if let Err(e) = signed_spend.verify(signed_spend.spent_tx_hash()) {
        return Err(Error::InvalidSpendSignature(format!(
            "while verifying spend for {:?}: {e:?}",
            signed_spend.dbc_id()
        )));
    }
    check_parent_spends(network, signed_spend).await?;
    check_for_double_spend(network, signed_spend).await?;

    Ok(())
}

/// Fetch all parent spends from the network and check them
/// they should all exist as valid spends for this current spend attempt to be valid
async fn check_parent_spends(network: &Network, signed_spend: &SignedSpend) -> Result<()> {
    trace!("Getting parent_spends for {:?}", signed_spend.dbc_id());
    let parent_spends = match get_parent_spends(network, &signed_spend.spent_tx()).await {
        Ok(parent_spends) => parent_spends,
        Err(e) => return Err(e)?,
    };

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

/// Fetch all parent spends from the network
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
        let parent_address = DbcAddress::from_dbc_id(&parent_input.dbc_id());
        // This call makes sure we get the same spend from all in the close group.
        // If we receive a spend here, it is assumed to be valid. But we will verify
        // that anyway, in the code right after this for loop.
        trace!("getting parent_spend for {:?}", parent_address.name());
        let parent_spend = get_network_valid_spend(network, parent_address).await?;
        trace!("got parent_spend for {:?}", parent_address.name());
        let _ = all_parent_spends.insert(parent_spend);
    }

    Ok(all_parent_spends)
}

/// Retrieve spends from the closest peers and checks if majority agrees on it
/// If majority agrees, return the agreed spend
async fn get_network_valid_spend(network: &Network, address: DbcAddress) -> Result<SignedSpend> {
    let spends = get_spend(network, address).await?;
    let valid_spends: Vec<_> = spends
        .iter()
        .filter(|signed_spend| signed_spend.verify(signed_spend.spent_tx_hash()).is_ok())
        .collect();

    if valid_spends.len() >= close_group_majority() {
        use itertools::*;
        let resp_count_by_spend: BTreeMap<&SignedSpend, usize> = valid_spends
            .clone()
            .into_iter()
            .map(|x| (x, 1))
            .into_group_map()
            .into_iter()
            .map(|(spend, vec_of_ones)| (spend, vec_of_ones.len()))
            .collect();

        if resp_count_by_spend.keys().len() > 1 {
            let mut proof = resp_count_by_spend.keys().take(2);
            if let (Some(spend_one), Some(spend_two)) = (proof.next(), proof.next()) {
                return Err(Error::DoubleSpendAttempt {
                    spend_one: Box::new(spend_one.to_owned().clone()),
                    spend_two: Box::new(spend_two.to_owned().clone()),
                })?;
            }
        }

        let majority_agreement = resp_count_by_spend
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(k, _)| k);

        if let Some(agreed_spend) = majority_agreement {
            return Ok(agreed_spend.clone());
        }
    }

    warn!(
        "The spend for addr: {address:?} is not recognised by majority of peers in its close group"
    );
    Err(Error::InsufficientValidSpendsFound(address))
}

/// Requests spends from the closest peers
async fn get_spend(network: &Network, address: DbcAddress) -> Result<Vec<SignedSpend>> {
    let request = Request::Query(Query::GetSpend(address));
    let responses = network.node_send_to_closest(&request).await.map_err(|e| {
        warn!("Error while fetching spends on the Network for {address:?}: {e:?}");
        Error::FailedToGetSpend(address)
    })?;

    // Get all Ok results of the expected response type `GetDbcSpend`.
    let mut double_spend_answer = None;
    let spends: Vec<_> = responses
        .iter()
        .flatten()
        .flat_map(|resp| {
            match resp {
                Response::Query(QueryResponse::GetDbcSpend(Ok(signed_spend))) => {
                    Some(signed_spend.clone())
                }
                Response::Query(QueryResponse::GetDbcSpend(Err(Error::DoubleSpendAttempt{ spend_one, spend_two }))) => {
                    if SpendBook::is_valid_double_spend(spend_one, spend_two) {
                        warn!("Double spend attempt reported by peer: {spend_one:?} and {spend_two:?}");
                        double_spend_answer = Some((spend_one, spend_two));
                    } else {
                        warn!("Ignoring invalid double spend reported by dirty liar peer");
                    }
                    None
                }
                Response::Query(QueryResponse::GetDbcSpend(Err(e))) => {
                    warn!("Peer sent us an error while getting spend from network: {e:?}");
                    None
                }
                _ => {
                    // TODO check what it means if we get a different response type
                    None
                }
            }
        })
        .collect();

    // check if peers reported double spend
    if let Some((spend_one, spend_two)) = double_spend_answer {
        return Err(Error::DoubleSpendAttempt {
            spend_one: spend_one.to_owned(),
            spend_two: spend_two.to_owned(),
        })?;
    }

    Ok(spends)
}
