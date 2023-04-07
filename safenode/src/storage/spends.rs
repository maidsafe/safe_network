// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::protocol::types::{
    address::DbcAddress,
    error::{Error, Result},
};

use sn_dbc::{DbcId, SignedSpend};

use clru::CLruCache;
use std::{
    collections::BTreeSet,
    fmt::{self, Display, Formatter},
    num::NonZeroUsize,
    sync::Arc,
};
use tokio::sync::RwLock;
use tracing::trace;
use xor_name::XorName;

/// We will store at most 50MiB of data in a SpendStorage instance.
const SPENDS_CACHE_SIZE: usize = 45 * 1024 * 1024;
const DOUBLE_SPENDS_CACHE_SIZE: usize = 5 * 1024 * 1024;

/// For every DbcId, there is a collection of transactions.
/// Every transaction has a set of peers who reported that they hold this transaction.
/// At a higher level, a peer will store a spend to `valid_spends` if the dbc checks out as valid, _and_ the parents of the dbc checks out as valid.
/// A peer will move a spend from `valid_spends` to `double_spends` if it receives another tx id for the same dbc id.
/// A peer will never again store such a spend to `valid_spends`.
type ValidSpends<V> = Arc<RwLock<CLruCache<DbcAddress, V>>>;
type DoubleSpends<V> = Arc<RwLock<CLruCache<DbcAddress, BTreeSet<V>>>>;

/// Storage of Dbc spends.
#[derive(Clone, Debug)]
pub(super) struct SpendStorage {
    valid_spends: ValidSpends<SignedSpend>,
    double_spends: DoubleSpends<SignedSpend>,
}

impl SpendStorage {
    // Read Spend from local store.
    pub(super) async fn get(&self, address: &DbcAddress) -> Result<SignedSpend> {
        trace!("Getting Spend: {address:?}");
        if let Some(spend) = self.valid_spends.read().await.peek(address) {
            Ok(spend.clone())
        } else {
            Err(Error::SpendNotFound(*address))
        }
    }

    /// We need to check that the parent is spent before
    /// we try add here.
    /// If a double spend attempt is detected, a `DoubleSpendAttempt` error
    /// will be returned including all the `SignedSpends`, for
    /// broadcasting to the other nodes.
    pub(super) async fn try_add(&self, signed_spend: &SignedSpend) -> Result<()> {
        // We want to return Result<(SignedSpend)> here, so that this node
        let address = dbc_address(signed_spend.dbc_id());
        // I was thinking that we perhaps can't hold a reference into the cache, as it could
        // deadlock if we want to write further down? Not sure if that would happen. To be confirmed.
        let mut double_spends_of_this_dbc = match self.double_spends.read().await.peek(&address) {
            Some(set) => set.clone(),
            None => BTreeSet::new(),
        };

        // Important: The spend id is from the spend hash. This makes sure
        // that a spend is compared based on both the `DbcTransaction` and the `DbcReason` being equal.
        let spend_id = get_spend_id(signed_spend.spend.hash());
        let tamper_attempted = match self.valid_spends.read().await.peek(&address) {
            Some(existing) => spend_id != get_spend_id(existing.spend.hash()),
            None => false,
        };
        let tamper_previously_detected = double_spends_of_this_dbc
            .iter()
            .any(|s| s.dbc_id() == signed_spend.dbc_id());

        if tamper_attempted || tamper_previously_detected {
            // The data argument, being a SignedSpend, I don't get where it comes from.
            let _replaced = self.double_spends.write().await.put_or_modify(
                address,
                |_addr, _proof| {
                    let _ = double_spends_of_this_dbc.insert(signed_spend.clone());
                    double_spends_of_this_dbc.clone()
                },
                |_a, map, _c| {
                    let _ = map.insert(signed_spend.clone());
                },
                signed_spend.clone(),
            );

            if tamper_attempted {
                // The spend is now permanently removed from the valid spends.
                self.valid_spends
                    .write()
                    .await
                    .retain(|key, _| key != &address);
            }

            return Err(Error::DoubleSpendAttempt(double_spends_of_this_dbc));
        }

        if self.valid_spends.read().await.peek(&address).is_none() {
            // will it deadlock here?
            let _ = self
                .valid_spends
                .write()
                .await
                .put(address, signed_spend.clone());
        }

        Ok(())
        // Ok(spend.clone())
    }

    /// When data is replicated to a new peer,
    /// it may contain double spends, and thus we need to add that here,
    /// so that we in the future can serve this info to Clients.
    #[allow(unused)] // to be used when we replicate data between nodes
    pub(super) async fn try_add_doubles(
        &self,
        received_double_spends: &BTreeSet<SignedSpend>,
    ) -> Result<()> {
        // Since the SignedSpends are in a BTreeSet, we know that they are different.
        // We will also check that we have more than one SignedSpend for each DbcId,
        // which would prove to us that there was a double spend attempt for that Dbc.
        let unique_ids: BTreeSet<_> = received_double_spends.iter().map(|s| s.dbc_id()).collect();

        for dbc_id in unique_ids {
            let copies: BTreeSet<_> = received_double_spends
                .iter()
                .map(|s| dbc_id == s.dbc_id())
                .collect();
            if copies.len() <= 1 {
                // We only add these to double_spends if there are two or more of them,
                // as otherwise we can't actually tell that it's a double spend attempt.
                continue;
            }
            let address = dbc_address(dbc_id);
            let _replaced = self.double_spends.write().await.put_or_modify(
                address,
                |_addr, double_spends| double_spends,
                |_a, map, double_spends| {
                    map.extend(double_spends);
                },
                received_double_spends.clone(),
            );
            // The spend is now permanently removed from the valid spends.
            self.valid_spends
                .write()
                .await
                .retain(|key, _| key != &address);
        }

        Ok(())
    }
}

impl Display for SpendStorage {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "SpendStorage")
    }
}

impl Default for SpendStorage {
    fn default() -> Self {
        let spend_capacity =
            NonZeroUsize::new(SPENDS_CACHE_SIZE).expect("Failed to create in-memory Spend cache.");
        let double_spend_capacity = NonZeroUsize::new(DOUBLE_SPENDS_CACHE_SIZE)
            .expect("Failed to create in-memory DoubleSpend cache");
        Self {
            valid_spends: Arc::new(RwLock::new(CLruCache::new(spend_capacity))),
            double_spends: Arc::new(RwLock::new(CLruCache::new(double_spend_capacity))),
        }
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

/// Still thinking of best location for this.
/// Wanted to make the DbcAddress take a dbc id actually..
fn get_spend_id(content_hash: sn_dbc::Hash) -> XorName {
    // TODO: XorName(*content_hash.slice())
    XorName::from_content(content_hash.as_ref())
}
