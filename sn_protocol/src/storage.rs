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

use core::fmt;
use exponential_backoff::Backoff;
use std::{num::NonZeroUsize, time::Duration};

pub use self::{
    address::{ChunkAddress, RegisterAddress, ScratchpadAddress, SpendAddress},
    chunks::Chunk,
    header::{try_deserialize_record, try_serialize_record, RecordHeader, RecordKind, RecordType},
    scratchpad::Scratchpad,
};

/// A strategy that translates into a configuration for exponential backoff.
/// The first retry is done after 2 seconds, after which the backoff is roughly doubled each time.
/// The interval does not go beyond 32 seconds. So the intervals increase from 2 to 4, to 8, to 16, to 32 seconds and
/// all attempts are made at most 32 seconds apart.
///
/// The exact timings depend on jitter, which is set to 0.2, meaning the intervals can deviate quite a bit
/// from the ones listed in the docs.
#[derive(Clone, Debug, Copy, Default)]
pub enum RetryStrategy {
    /// Attempt once (no retries)
    None,
    /// Retry 3 times (waits 2s, 4s and lastly 8s; max total time ~14s)
    Quick,
    /// Retry 5 times (waits 2s, 4s, 8s, 16s and lastly 32s; max total time ~62s)
    #[default]
    Balanced,
    /// Retry 9 times (waits 2s, 4s, 8s, 16s, 32s, 32s, 32s, 32s and lastly 32s; max total time ~190s)
    Persistent,
    /// Attempt a specific number of times
    N(NonZeroUsize),
}

impl RetryStrategy {
    pub fn attempts(&self) -> usize {
        match self {
            RetryStrategy::None => 1,
            RetryStrategy::Quick => 4,
            RetryStrategy::Balanced => 6,
            RetryStrategy::Persistent => 10,
            RetryStrategy::N(x) => x.get(),
        }
    }

    pub fn backoff(&self) -> Backoff {
        let mut backoff = Backoff::new(
            self.attempts() as u32,
            Duration::from_secs(1), // First interval is double of this (see https://github.com/yoshuawuyts/exponential-backoff/issues/23)
            Some(Duration::from_secs(32)),
        );
        backoff.set_factor(2); // Default.
        backoff.set_jitter(0.2); // Default is 0.3.
        backoff
    }
}

impl fmt::Display for RetryStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[test]
fn verify_retry_strategy_intervals() {
    let intervals = |strategy: RetryStrategy| -> Vec<u32> {
        let mut backoff = strategy.backoff();
        backoff.set_jitter(0.01); // Make intervals deterministic.
        backoff
            .into_iter()
            .flatten()
            .map(|duration| duration.as_secs_f64().round() as u32)
            .collect()
    };

    assert_eq!(intervals(RetryStrategy::None), Vec::<u32>::new());
    assert_eq!(intervals(RetryStrategy::Quick), vec![2, 4, 8]);
    assert_eq!(intervals(RetryStrategy::Balanced), vec![2, 4, 8, 16, 32]);
    assert_eq!(
        intervals(RetryStrategy::Persistent),
        vec![2, 4, 8, 16, 32, 32, 32, 32, 32]
    );
    assert_eq!(
        intervals(RetryStrategy::N(NonZeroUsize::new(12).unwrap())),
        vec![2, 4, 8, 16, 32, 32, 32, 32, 32, 32, 32]
    );
}
