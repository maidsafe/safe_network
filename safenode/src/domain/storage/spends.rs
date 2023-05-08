// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

//! For every DbcId, there is a collection of transactions.
//! Every transaction has a set of peers who reported that they hold this transaction.
//! At a higher level, a peer will store a spend to `valid_spends` if the dbc checks out as valid, _and_ the parents of the dbc checks out as valid.
//! A peer will move a spend from `valid_spends` to `double_spends` if it receives another tx id for the same dbc id.
//! A peer will never again store such a spend to `valid_spends`.

use crate::protocol::{
    error::StorageError,
    storage::{dbc_address, DbcAddress},
};

use super::{prefix_tree_path, Result};

use sn_dbc::{DbcId, SignedSpend};

use bincode::{deserialize, serialize};
use std::{
    fmt::{self, Display, Formatter},
    path::{Path, PathBuf},
};
use tokio::{
    fs::{create_dir_all, read, remove_file, File},
    io::AsyncWriteExt,
};
use tracing::trace;

const VALID_SPENDS_STORE_DIR_NAME: &str = "valid_spends";
const DOUBLE_SPENDS_STORE_DIR_NAME: &str = "double_spends";

/// Storage of Dbc spends.
///
/// NB: The used space measurement is just an appromixation, and is not exact.
/// Later, when all data types have this, we can verify that it is not wildly different.
#[derive(Clone, Debug)]
pub(crate) struct SpendStorage {
    valid_spends_path: PathBuf,
    double_spends_path: PathBuf,
}

impl SpendStorage {
    /// Create a new instance of `SpendStorage`.
    pub(crate) fn new(path: &Path) -> Self {
        Self {
            valid_spends_path: path.join(VALID_SPENDS_STORE_DIR_NAME),
            double_spends_path: path.join(DOUBLE_SPENDS_STORE_DIR_NAME),
        }
    }

    /// Returns if the spend already exists.
    pub(crate) fn exists(&mut self, dbc_id: &DbcId) -> Result<bool> {
        let address = dbc_address(dbc_id);
        let filepath = self.address_to_filepath(&address, &self.valid_spends_path)?;
        Ok(filepath.exists())
    }

    // Read Spend from local store.
    pub(crate) async fn get(&self, address: &DbcAddress) -> Result<SignedSpend> {
        let file_path = self.address_to_filepath(address, &self.valid_spends_path)?;
        trace!("Getting Spend: {address:?} from {:?}", file_path);
        match read(file_path).await {
            Ok(bytes) => {
                let spend: SignedSpend = deserialize(&bytes).map_err(|err| {
                    warn!("We couldn't deserialise the SignedSpend read from disk: {err:?}");
                    StorageError::SpendNotFound(*address)
                })?;
                if address == &dbc_address(spend.dbc_id()) {
                    Ok(spend)
                } else {
                    // This can happen if the content read is empty, or incomplete,
                    // possibly due to an issue with the OS synchronising to disk,
                    // resulting in a mismatch with recreated address of the Spend.
                    Err(StorageError::SpendNotFound(*address))
                }
            }
            Err(_) => Err(StorageError::SpendNotFound(*address)),
        }
    }

    /// Try store a spend to local file system.
    ///
    /// We need to check that the parent is spent before
    /// we try add here.
    /// If a double spend attempt is detected, a `DoubleSpendAttempt` error
    /// will be returned including all the `SignedSpends`, for
    /// broadcasting to the other nodes.
    /// NOTE: The `&mut self` signature is necessary to prevent race conditions
    /// and double spent attempts to be missed (as the validation and adding
    /// could otherwise happen in parallel in different threads.)
    pub(crate) async fn try_add(&mut self, signed_spend: &SignedSpend) -> Result<bool> {
        self.validate(signed_spend).await?;
        self.add(signed_spend).await
    }

    /// Validates a spend without adding it to the storage.
    /// If it however is detected as a double spend, that fact is recorded immediately,
    /// and an error returned.
    /// NOTE: The `&mut self` signature is necessary to prevent race conditions
    /// and double spent attempts to be missed (as the validation and adding
    /// could otherwise happen in parallel in different threads.)
    pub(crate) async fn validate(&mut self, signed_spend: &SignedSpend) -> Result<()> {
        let address = dbc_address(signed_spend.dbc_id());
        if self.try_get_double_spend(&address).await {
            return Err(StorageError::AlreadyMarkedAsDoubleSpend(address));
        }

        if let Ok(existing) = self.get(&address).await {
            let tamper_attempted = signed_spend.spend.hash() != existing.spend.hash();
            if tamper_attempted {
                trace!("Tamper attempt detected, just rejecting the request.");
                return Err(StorageError::AlreadyExists(address));
                // // We don't error if the double spend failed, as we rather want to
                // // announce the double spend attempt to close group. TODO: how to handle the error then?
                // self.try_store_double_spend(&existing, signed_spend).await?;

                // // The spend is now permanently removed from the valid spends.
                // // We don't error if the remove failed, as we rather want to
                // // announce the double spend attempt to close group.
                // // The double spend will still be detected by querying for the spend.
                // self.remove(&address, &self.valid_spends_path).await?;

                // return Err(StorageError::DoubleSpendAttempt {
                //     new: Box::new(signed_spend.clone()),
                //     existing: Box::new(existing),
                // });
            }
        }

        // This hash input is pointless, since it will compare with
        // the same hash in the verify fn.
        // It does however verify that the derived key corresponding to
        // the dbc id signed this spend.
        signed_spend
            .verify(signed_spend.dst_tx_hash())
            .map_err(|err| StorageError::InvalidSpendSignature(err.to_string()))?;
        // TODO: We want to verify the transaction somehow as well..
        // signed_spend.spend.tx.verify(blinded_amounts)

        Ok(())
    }

    /// When data is replicated to a new peer,
    /// it may contain double spends, and thus we need to add that here,
    /// so that we in the future can serve this info to Clients.
    /// NOTE: The `&mut self` signature is necessary to prevent race conditions
    /// and double spent attempts to be missed (as the validation and adding
    /// could otherwise happen in parallel in different threads.)
    pub(crate) async fn try_add_double(
        &mut self,
        a_spend: &SignedSpend,
        b_spend: &SignedSpend,
    ) -> Result<()> {
        let different_id = a_spend.dbc_id() != b_spend.dbc_id();
        let a_hash = sn_dbc::Hash::hash(&a_spend.to_bytes());
        let b_hash = sn_dbc::Hash::hash(&b_spend.to_bytes());
        let same_hash = a_hash == b_hash;

        // A node could erroneously send a notification of a double spend attempt,
        // so, we need to validate that.
        if different_id || same_hash {
            // If the ids are different, then this is not a double spend attempt.
            // A double spend attempt is when the contents (the tx) of two spends
            // with same id are detected as being different.
            // That means that if the ids are the same, and the hashes the same, then
            // it isn't a double spend attempt either!
            warn!(
                "We were notified about a double spend attempt, but they were \
                for different dbcs. One: {a_spend:?}, another: {b_spend:?}"
            );
            return Ok(());
        }

        if self.is_unspendable(a_spend.dbc_id()).await {
            return Ok(());
        }

        let address = dbc_address(a_spend.dbc_id());

        self.try_store_double_spend(a_spend, b_spend).await?;

        // The spend is now permanently removed from the valid spends.
        // We don't error if the remove failed (not found or whatever)
        // as that isn't a problem.
        let _ = self.remove(&address, &self.valid_spends_path).await;

        Ok(())
    }

    /// Returns true if it was stored, and false if it already existed.
    async fn add(&mut self, signed_spend: &SignedSpend) -> Result<bool> {
        if self.exists(signed_spend.dbc_id())? {
            return Ok(false);
        }

        let address = dbc_address(signed_spend.dbc_id());
        let filepath = self.address_to_filepath(&address, &self.valid_spends_path)?;

        // Store the spend to local file system.
        trace!("Storing spend {address:?}.");
        if let Some(dirs) = filepath.parent() {
            create_dir_all(dirs).await.map_err(|err| {
                warn!("We couldn't create dir structure to write spend to disk: {err:?}");
                StorageError::SpendNotStored(address)
            })?;
        }

        let mut file = File::create(filepath).await.map_err(|err| {
            warn!("We couldn't create file to write spend to disk: {err:?}");
            StorageError::SpendNotStored(address)
        })?;

        let bytes = serialize(signed_spend).map_err(|err| {
            warn!("We couldn't serialise the spend to write it to disk: {err:?}");
            StorageError::SpendNotStored(address)
        })?;
        file.write_all(&bytes).await.map_err(|err| {
            warn!("We couldn't write the serialised spend to disk: {err:?}");
            StorageError::SpendNotStored(address)
        })?;

        // Sync up OS data to disk to reduce the chances of
        // concurrent reading failing by reading an empty/incomplete file.
        if let Err(err) = file.sync_data().await {
            warn!("We couldn't sync spend file to disk: {err:?}");
        }

        trace!("Stored new spend {address:?}.");
        Ok(true)
    }

    /// Checks if the given DbcId is unspendable.
    async fn is_unspendable(&self, dbc_id: &DbcId) -> bool {
        let address = dbc_address(dbc_id);
        self.try_get_double_spend(&address).await
    }

    fn address_to_filepath(&self, addr: &DbcAddress, root: &Path) -> Result<PathBuf> {
        let xorname = *addr.name();
        let path = prefix_tree_path(root, xorname);
        let filename = hex::encode(xorname);
        Ok(path.join(filename))
    }

    async fn remove(&self, address: &DbcAddress, root: &Path) -> Result<()> {
        debug!("Removing spend, {:?}", address);
        let file_path = self.address_to_filepath(address, root)?;
        if let Err(err) = remove_file(file_path).await {
            warn!("We couldn't remove DBC from disk: {err:?}");
        }
        Ok(())
    }

    async fn try_get_double_spend(&self, address: &DbcAddress) -> bool {
        trace!("Looking for double spend: {address:?}");
        let file_path =
            if let Ok(path) = self.address_to_filepath(address, &self.double_spends_path) {
                path
            } else {
                return false;
            };

        match read(file_path).await {
            Err(_) => false,
            Ok(bytes) => match deserialize::<(SignedSpend, SignedSpend)>(&bytes) {
                Err(err) => {
                    warn!("We couldn't deserialise the double SignedSpend read from disk: {err:?}");
                    false
                }
                Ok((a_spend, b_spend)) => {
                    if a_spend.dbc_id() != b_spend.dbc_id() {
                        return false;
                    }

                    // If the content read is empty, or incomplete,
                    // possibly due to an issue with the OS synchronising to disk,
                    // it can result in a mismatch with recreated address of the Spend.
                    address == &dbc_address(a_spend.dbc_id())
                }
            },
        }
    }

    async fn try_store_double_spend(
        &mut self,
        a_spend: &SignedSpend,
        b_spend: &SignedSpend,
    ) -> Result<()> {
        // They have the same dbc id, so we can use either.
        let address = dbc_address(a_spend.dbc_id());

        let filepath = self.address_to_filepath(&address, &self.double_spends_path)?;

        if filepath.exists() {
            return Ok(());
        }

        // Store the double spend to local file system.
        trace!("Storing double spend {address:?}.");
        if let Some(dirs) = filepath.parent() {
            create_dir_all(dirs).await.map_err(|err| {
                warn!("We couldn't create dir structure to write double spend to disk: {err:?}");
                StorageError::SpendNotStored(address)
            })?;
        }

        let mut file = File::create(filepath).await.map_err(|err| {
            warn!("We couldn't create file to write double spend to disk: {err:?}");
            StorageError::SpendNotStored(address)
        })?;

        let bytes = serialize(&(a_spend, b_spend)).map_err(|err| {
            warn!("We couldn't serialise the double spend to write them to disk: {err:?}");
            StorageError::SpendNotStored(address)
        })?;
        file.write_all(&bytes).await.map_err(|err| {
            warn!("We couldn't write the serialised double spend to disk: {err:?}");
            StorageError::SpendNotStored(address)
        })?;

        // Sync up OS data to disk to reduce the chances of
        // concurrent reading failing by reading an empty/incomplete file.
        if let Err(err) = file.sync_data().await {
            warn!("We couldn't sync double spend file to disk: {err:?}");
        }

        trace!("Stored double spend {address:?}.");

        Ok(())
    }
}

impl Display for SpendStorage {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "SpendStorage")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::dbc_genesis::{create_first_dbc_from_key, split};
    use assert_fs::TempDir;
    use sn_dbc::MainKey;

    #[tokio::test]
    async fn write_and_read_100_spends() {
        // Test that a range of different spends can be stored and read as expected.
        let number_of_spends = 100;
        let mut storage = init_file_store();

        let key = MainKey::random();
        let dbc = create_first_dbc_from_key(&key).expect("First dbc creation to succeed.");
        let dbcs = split(&dbc, &key, number_of_spends).expect("Split to succeed.");
        let spends: Vec<_> = dbcs
            .into_iter()
            .flat_map(|(dbc, _)| dbc.signed_spends)
            .collect();

        assert_eq!(spends.len(), number_of_spends);

        for spend in spends {
            let _ = storage
                .try_add(&spend)
                .await
                .expect("Failed to write spend.");
            let read_spend = storage
                .get(&dbc_address(spend.dbc_id()))
                .await
                .expect("Failed to read spend.");

            assert_eq!(spend.to_bytes(), read_spend.to_bytes());
        }
    }

    #[tokio::test]
    async fn try_add_is_idempotent() {
        let mut storage = init_file_store();
        let key = MainKey::random();
        let src_dbc = create_first_dbc_from_key(&key).expect("First dbc creation to succeed.");

        let dbc = split(&src_dbc, &key, 1).expect("Split to succeed.");
        let (dbc, _) = &dbc[0];
        let spend = dbc.signed_spends.last().expect("Should contain a spend.");

        // Adding the exact same spend is idempotent.
        let _ = storage
            .try_add(spend)
            .await
            .expect("First spend should be added.");
        let _ = storage
            .try_add(spend)
            .await
            .expect("The exact same spend should be added.");
    }

    #[ignore = "We have temporarily disabled the punishment of double spends. NB it is still detected and hindered, just not punished."]
    #[tokio::test]
    async fn double_spend_attempt_is_detected() {
        let mut storage = init_file_store();
        let key = MainKey::random();
        let src_dbc = create_first_dbc_from_key(&key).expect("First dbc creation to succeed.");

        let dbc = split(&src_dbc, &key, 1).expect("Split to succeed.");
        let (dbc, _) = &dbc[0];
        let spend = dbc.signed_spends.last().expect("Should contain a spend.");

        let _ = storage
            .try_add(spend)
            .await
            .expect("First spend should be added.");

        // The tampered spend will have the same id and src, but another another dst transaction.
        let mut tampered_spend = spend.clone();
        let double_spend_dbc = split(&src_dbc, &key, 1).expect("Split to succeed.");
        let (double_spend_dbc, _) = &double_spend_dbc[0];
        let double_spend = double_spend_dbc
            .signed_spends
            .last()
            .expect("Should contain a spend.");
        tampered_spend.spend.dst_tx = double_spend.spend.dst_tx.clone();

        match storage.try_add(&tampered_spend).await {
            Ok(_) => panic!("Double spend should not be allowed."),
            Err(super::StorageError::DoubleSpendAttempt { new, existing }) => {
                assert_eq!(new.to_bytes(), tampered_spend.to_bytes());
                assert_eq!(existing.to_bytes(), spend.to_bytes());
            }
            Err(other) => panic!("Unexpected error: {:?}", other),
        }

        // Both of these spends result as already marked
        // as double spend, when trying to add them again.
        match storage.try_add(spend).await {
            Ok(_) => panic!("Double spend should not be allowed."),
            Err(super::StorageError::AlreadyMarkedAsDoubleSpend(address)) => {
                assert_eq!(dbc_address(spend.dbc_id()), address);
            }
            Err(other) => panic!("Unexpected error: {:?}", other),
        }
        match storage.try_add(&tampered_spend).await {
            Ok(_) => panic!("Double spend should not be allowed."),
            Err(super::StorageError::AlreadyMarkedAsDoubleSpend(address)) => {
                assert_eq!(dbc_address(spend.dbc_id()), address);
            }
            Err(other) => panic!("Unexpected error: {:?}", other),
        }
    }

    #[tokio::test]
    async fn try_add_double_is_idempotent() {
        let mut storage = init_file_store();
        let key = MainKey::random();
        let src_dbc = create_first_dbc_from_key(&key).expect("First dbc creation to succeed.");

        let dbc = split(&src_dbc, &key, 1).expect("Split to succeed.");
        let (dbc, _) = &dbc[0];
        let a_spend = dbc.signed_spends.last().expect("Should contain a spend.");

        // The tampered spend will have the same id and src, but another another dst transaction.
        let mut b_spend = a_spend.clone();
        let dbc = split(&src_dbc, &key, 1).expect("Split to succeed.");
        let (dbc, _) = &dbc[0];
        let other_spend = dbc.signed_spends.last().expect("Should contain a spend.");
        b_spend.spend.dst_tx = other_spend.spend.dst_tx.clone();

        assert!(storage.validate(a_spend).await.is_ok());
        // This is a malformed spend, and would always fail.
        assert!(storage.validate(&b_spend).await.is_err());

        match storage.try_add_double(a_spend, &b_spend).await {
            Ok(_) => (),
            Err(err) => panic!("Did not expect an error: {err:?}!"),
        }

        assert!(storage.validate(a_spend).await.is_err());
        assert!(storage.validate(&b_spend).await.is_err());

        match storage.try_add_double(a_spend, &b_spend).await {
            Ok(_) => (),
            Err(_) => panic!("Did not expect an error!"),
        }

        assert!(storage.validate(a_spend).await.is_err());
        assert!(storage.validate(&b_spend).await.is_err());
    }

    #[tokio::test]
    async fn try_add_fails_after_added_double_spend() {
        let mut storage = init_file_store();
        let key = MainKey::random();
        let src_dbc = create_first_dbc_from_key(&key).expect("First dbc creation to succeed.");

        let dbc = split(&src_dbc, &key, 1).expect("Split to succeed.");
        let (dbc, _) = &dbc[0];
        let a_spend = dbc.signed_spends.last().expect("Should contain a spend.");

        // The tampered spend will have the same id and src, but another another dst transaction.
        let mut b_spend = a_spend.clone();
        let dbc = split(&src_dbc, &key, 1).expect("Split to succeed.");
        let (dbc, _) = &dbc[0];
        let other_spend = dbc.signed_spends.last().expect("Should contain a spend.");
        b_spend.spend.dst_tx = other_spend.spend.dst_tx.clone();

        match storage.try_add_double(a_spend, &b_spend).await {
            Ok(_) => (),
            Err(_) => panic!("Did not expect an error!"),
        }

        // Both of these spends result as already marked
        // as double spend, when trying to add them again.
        match storage.try_add(a_spend).await {
            Ok(_) => panic!("Double spend should not be allowed."),
            Err(super::StorageError::AlreadyMarkedAsDoubleSpend(address)) => {
                assert_eq!(dbc_address(a_spend.dbc_id()), address);
            }
            Err(other) => panic!("Unexpected error: {:?}", other),
        }
        match storage.try_add(&b_spend).await {
            Ok(_) => panic!("Double spend should not be allowed."),
            Err(super::StorageError::AlreadyMarkedAsDoubleSpend(address)) => {
                assert_eq!(dbc_address(b_spend.dbc_id()), address);
            }
            Err(other) => panic!("Unexpected error: {:?}", other),
        }
    }

    fn init_file_store() -> SpendStorage {
        let root = TempDir::new().expect("Should be able to create a temp dir.");
        SpendStorage::new(root.path())
    }
}
