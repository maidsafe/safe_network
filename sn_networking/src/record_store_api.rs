// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use libp2p::kad::{store::Result, KBucketDistance as Distance, Record, RecordKey};
use sn_dbc::Token;
use sn_protocol::NetworkAddress;
use std::collections::HashSet;

pub trait RecordStoreAPI {
    /// Returns `true` if the `Key` is present locally
    fn contains(&self, key: &RecordKey) -> bool;

    /// Returns the set of `NetworkAddress::RecordKey` held by the store
    /// Use `record_addresses_ref` to get a borrowed type
    fn record_addresses(&self) -> HashSet<NetworkAddress>;

    /// Returns the reference to the set of `NetworkAddress::RecordKey` held by the store
    #[allow(clippy::mutable_key_type)]
    fn record_addresses_ref(&self) -> &HashSet<RecordKey>;

    /// Warning: PUTs a `Record` to the store without validation
    /// Should be used in context where the `Record` is trusted
    fn put_verified(&mut self, r: Record) -> Result<()>;

    /// Calculate the cost to store data for our current store state
    fn store_cost(&self) -> Token;

    /// Setup the distance range.
    fn set_distance_range(&mut self, distance_range: Distance);
}
