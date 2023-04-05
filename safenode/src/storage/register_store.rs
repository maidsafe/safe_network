// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::protocol::{
    messages::RegisterCmd,
    types::{address::RegisterAddress, register::Register},
};

use super::{Error, Result};

use clru::CLruCache;
use std::{num::NonZeroUsize, sync::Arc};
use tokio::sync::RwLock;
use tracing::trace;

const REGISTERS_CACHE_SIZE: usize = 20 * 1024 * 1024;

pub(super) type RegisterLog = Vec<RegisterCmd>;

#[derive(Clone, Debug)]
pub(super) struct StoredRegister {
    pub(super) state: Option<Register>,
    pub(super) op_log: RegisterLog,
}

/// A store for Registers
#[derive(Clone)]
pub(super) struct RegisterStore {
    cache: Arc<RwLock<CLruCache<RegisterAddress, StoredRegister>>>,
}

impl Default for RegisterStore {
    fn default() -> Self {
        let capacity = NonZeroUsize::new(REGISTERS_CACHE_SIZE)
            .expect("Failed to create in-memory Registers storage");
        Self {
            cache: Arc::new(RwLock::new(CLruCache::new(capacity))),
        }
    }
}

impl RegisterStore {
    #[cfg(test)]
    pub(super) async fn addrs(&self) -> Vec<RegisterAddress> {
        self.cache
            .read()
            .await
            .iter()
            .map(|(addr, _)| *addr)
            .collect()
    }

    #[allow(dead_code)]
    pub(super) async fn remove(&self, address: &RegisterAddress) -> Result<()> {
        trace!("Removing Register: {address:?}");
        if self.cache.write().await.pop(address).is_some() {
            Ok(())
        } else {
            Err(Error::RegisterNotFound(*address))
        }
    }

    /// Opens the log of RegisterCmds for a given Register address.
    /// Creates a new log if no data is found
    pub(super) async fn get(&self, address: &RegisterAddress) -> StoredRegister {
        trace!("Getting Register ops log: {address:?}");
        if let Some(stored_reg) = self.cache.read().await.peek(address) {
            stored_reg.clone()
        } else {
            StoredRegister {
                state: None,
                op_log: RegisterLog::new(),
            }
        }
    }

    /// Persists a RegisterLog
    pub(super) async fn store_register_ops_log(
        &self,
        _log: &RegisterLog, // we'll need to write these ops to disk when disk store is implemented
        reg: StoredRegister,
        address: RegisterAddress,
    ) -> Result<()> {
        let log_len = reg.op_log.len();
        trace!("Storing Register ops log with {log_len} cmd/s: {address:?}",);

        let _ = self.cache.write().await.try_put_or_modify(
            address,
            |_, _| Ok::<StoredRegister, Error>(reg.clone()),
            |_, stored_reg, _| {
                *stored_reg = reg.clone();
                Ok(())
            },
            (),
        )?;

        trace!("Register ops log of {log_len} cmd/s stored successfully: {address:?}",);
        Ok(())
    }
}
