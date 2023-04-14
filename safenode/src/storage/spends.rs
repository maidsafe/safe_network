// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    protocol::types::address::DbcAddress,
    storage::used_space::UsedSpace,
    transfers::{Error, Result},
};

use sn_dbc::{DbcId, SignedSpend};

use std::{
    collections::BTreeMap,
    fmt::{self, Display, Formatter},
    sync::Arc,
};
use tokio::sync::RwLock;
use tracing::trace;
use xor_name::XorName;

/// We will store at most 50MiB of data in a SpendStorage instance.
const VALID_SPENDS_CACHE_SIZE: usize = 45 * 1024 * 1024;
const DOUBLE_SPENDS_CACHE_SIZE: usize = 5 * 1024 * 1024;

/// For every DbcId, there is a collection of transactions.
/// Every transaction has a set of peers who reported that they hold this transaction.
/// At a higher level, a peer will store a spend to `valid_spends` if the dbc checks out as valid, _and_ the parents of the dbc checks out as valid.
/// A peer will move a spend from `valid_spends` to `double_spends` if it receives another tx id for the same dbc id.
/// A peer will never again store such a spend to `valid_spends`.
type ValidSpends<V> = Arc<RwLock<BTreeMap<DbcAddress, V>>>;
type DoubleSpends<V> = Arc<RwLock<BTreeMap<DbcAddress, (V, V)>>>;

/// Storage of Dbc spends.
///
/// NB: The used space measurement is just an appromixation, and is not exact.
/// Later, when all data types have this, we can verify that it is not wildly different.
#[derive(Clone, Debug)]
pub(crate) struct SpendStorage {
    valid_spends: ValidSpends<SignedSpend>,
    double_spends: DoubleSpends<SignedSpend>,
    valid_spends_cache_size: UsedSpace,
    double_spends_cache_size: UsedSpace,
}

impl SpendStorage {
    pub(crate) fn new() -> Self {
        Self {
            valid_spends: Arc::new(RwLock::new(BTreeMap::new())),
            double_spends: Arc::new(RwLock::new(BTreeMap::new())),
            valid_spends_cache_size: UsedSpace::new(VALID_SPENDS_CACHE_SIZE),
            double_spends_cache_size: UsedSpace::new(DOUBLE_SPENDS_CACHE_SIZE),
        }
    }

    // Read Spend from local store.
    pub(crate) async fn get(&self, address: DbcAddress) -> Result<SignedSpend> {
        trace!("Getting Spend: {address:?}");
        if let Some(spend) = self.valid_spends.read().await.get(&address) {
            Ok(spend.clone())
        } else {
            Err(Error::SpendNotFound(address))
        }
    }

    /// We need to check that the parent is spent before
    /// we try add here.
    /// If a double spend attempt is detected, a `DoubleSpendAttempt` error
    /// will be returned including all the `SignedSpends`, for
    /// broadcasting to the other nodes.
    pub(crate) async fn try_add(&self, signed_spend: &SignedSpend) -> Result<()> {
        self.validate(signed_spend).await?;

        let size_of_new = std::mem::size_of_val(signed_spend);
        let address = dbc_address(signed_spend.dbc_id());

        let mut valid_spends = self.valid_spends.write().await;

        let replaced = valid_spends.insert(address, signed_spend.clone());
        if replaced.is_none() {
            self.valid_spends_cache_size.increase(size_of_new);
        }

        Ok(())
    }

    /// Validates a spend without adding it to the storage.
    /// If it however is detected as a double spend, that fact is recorded immediately,
    /// and an error returned.
    pub(crate) async fn validate(&self, signed_spend: &SignedSpend) -> Result<()> {
        if self.is_unspendable(signed_spend.dbc_id()).await {
            return Ok(()); // Already unspendable, so we don't care about this spend.
        }

        let size_of_new = std::mem::size_of_val(signed_spend);
        if !self.valid_spends_cache_size.can_add(size_of_new) {
            return Err(Error::NotEnoughSpace); // We don't have space for this spend.
        }

        let address = dbc_address(signed_spend.dbc_id());

        // The spend id is from the spend hash. That makes sure that a spend is compared based
        // on all of `DbcTransaction`, `DbcReason`, `DbcId` and `BlindedAmount` being equal.
        let mut valid_spends = self.valid_spends.write().await;
        if let Some(existing) = valid_spends.get(&address).cloned() {
            let tamper_attempted = signed_spend.spend.hash() != existing.spend.hash();
            if tamper_attempted {
                if !self.double_spends_cache_size.can_add(size_of_new) {
                    return Err(Error::NotEnoughSpace); // We don't have space for this operation.
                }

                let mut double_spends = self.double_spends.write().await;
                let replaced =
                    double_spends.insert(address, (existing.clone(), signed_spend.clone()));

                let size_of_existing = std::mem::size_of_val(&existing);
                if replaced.is_none() {
                    self.double_spends_cache_size
                        .increase(size_of_new + size_of_existing);
                }

                // The spend is now permanently removed from the valid spends.
                let removed = valid_spends.remove(&address);
                if removed.is_some() {
                    self.valid_spends_cache_size.decrease(size_of_existing);
                }

                return Err(Error::DoubleSpendAttempt {
                    new: Box::new(signed_spend.clone()),
                    existing: Box::new(existing.clone()),
                });
            }
        };

        // This hash input is pointless, since it will compare with
        // the same hash in the verify fn.
        // It does however verify that the derived key corresponding to
        // the dbc id signed this spend.
        signed_spend.verify(signed_spend.dst_tx_hash())?;
        // TODO: We want to verify the transaction somehow as well..
        // signed_spend.spend.tx.verify(blinded_amounts)

        Ok(())
    }

    /// When data is replicated to a new peer,
    /// it may contain double spends, and thus we need to add that here,
    /// so that we in the future can serve this info to Clients.
    pub(crate) async fn try_add_double(
        &self,
        a_spend: &SignedSpend,
        b_spend: &SignedSpend,
    ) -> Result<()> {
        let different_id = a_spend.dbc_id() != b_spend.dbc_id();
        let a_hash = sn_dbc::Hash::hash(&a_spend.to_bytes());
        let b_hash = sn_dbc::Hash::hash(&b_spend.to_bytes());
        let same_hash = a_hash == b_hash;

        if different_id || same_hash {
            return Err(Error::NotADoubleSpendAttempt(
                Box::new(a_spend.clone()),
                Box::new(b_spend.clone()),
            ));
        }

        if self.is_unspendable(a_spend.dbc_id()).await {
            return Ok(());
        }

        let size_of_spends = std::mem::size_of_val(&(a_spend, b_spend));
        if !self.double_spends_cache_size.can_add(size_of_spends) {
            return Err(Error::NotEnoughSpace); // We don't have space for this spend.
        }

        let address = dbc_address(a_spend.dbc_id());

        let mut double_spends = self.double_spends.write().await;
        let replaced = double_spends.insert(address, (a_spend.clone(), b_spend.clone()));
        if replaced.is_none() {
            // This would only be reached if a double spend was registered
            // in between the call to `is_unspendable` and the call to `double_spends.insert`.
            self.double_spends_cache_size.increase(size_of_spends);
        }

        // The spend is now permanently removed from the valid spends.
        let mut valid_spends = self.valid_spends.write().await;
        if let Some(removed) = valid_spends.remove(&address) {
            self.valid_spends_cache_size
                .decrease(std::mem::size_of_val(&removed));
        }

        Ok(())
    }

    /// Checks if the given DbcId is unspendable.
    async fn is_unspendable(&self, dbc_id: &DbcId) -> bool {
        let address = dbc_address(dbc_id);
        self.double_spends.read().await.contains_key(&address)
    }
}

impl Display for SpendStorage {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "SpendStorage")
    }
}

impl Default for SpendStorage {
    fn default() -> Self {
        Self::new()
    }
}

/// Still thinking of best location for this.
/// Wanted to make the DbcAddress take a dbc id actually..
fn dbc_address(dbc_id: &DbcId) -> DbcAddress {
    DbcAddress::new(get_dbc_name(dbc_id))
}

/// Still thinking of best location for this.
/// Wanted to make the DbcAddress take a dbc id actually..
fn get_dbc_name(dbc_id: &DbcId) -> XorName {
    XorName::from_content(&dbc_id.to_bytes())
}
