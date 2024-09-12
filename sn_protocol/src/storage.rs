// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod address;
mod chunks;
mod header;
mod scratchpad;

use crate::error::Error;
use core::fmt;
use std::{str::FromStr, time::Duration};

pub use self::{
    address::{ChunkAddress, RegisterAddress, ScratchpadAddress, SpendAddress},
    chunks::Chunk,
    header::{try_deserialize_record, try_serialize_record, RecordHeader, RecordKind, RecordType},
    scratchpad::Scratchpad,
};

/// Represents the strategy for retrying operations. This encapsulates both the duration it may take for an operation to
/// complete or the retry attempts that it may take. This allows the retry of each operation, e.g., PUT/GET of
/// Chunk/Registers/Spend to be more flexible.
///
/// The Duration/Attempts is chosen based on the internal logic.
#[derive(Clone, Debug, Copy)]
pub enum RetryStrategy {
    /// Quick: Resolves to a 15-second wait or 1 retry attempt.
    Quick,
    /// Balanced: Resolves to a 60-second wait or 3 retry attempt.
    Balanced,
    /// Persistent: Resolves to a 180-second wait or 6 retry attempt.
    Persistent,
}

impl RetryStrategy {
    pub fn get_duration(&self) -> Duration {
        match self {
            RetryStrategy::Quick => Duration::from_secs(15),
            RetryStrategy::Balanced => Duration::from_secs(60),
            RetryStrategy::Persistent => Duration::from_secs(180),
        }
    }

    pub fn get_count(&self) -> usize {
        match self {
            RetryStrategy::Quick => 1,
            RetryStrategy::Balanced => 3,
            RetryStrategy::Persistent => 6,
        }
    }
}

impl FromStr for RetryStrategy {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "quick" => Ok(RetryStrategy::Quick),
            "balanced" => Ok(RetryStrategy::Balanced),
            "persistent" => Ok(RetryStrategy::Persistent),
            _ => Err(Error::ParseRetryStrategyError),
        }
    }
}

impl fmt::Display for RetryStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
