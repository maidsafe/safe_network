// Copyright (c) 2023, MaidSafe.
// All rights reserved.
//
// This SAFE Network Software is licensed under the BSD-3-Clause license.
// Please see the LICENSE file for more details.

use crate::{Hash, Nano};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct FeeOutput {
    /// The id is expected (in order to be accepted by the network) to be built from: hash(root_hash + inputs' ids).
    /// This requirement makes it possible for this output to be used as an input in a rewards/farming
    /// claiming Tx, by making its spend location deterministic, analogous to how any other output
    /// is spent using its id to determine the location to store the signed spend.
    pub id: Hash,
    /// Amount being paid as storage fee to the network.
    pub token: Nano,
    /// The root hash of the proof's Merkletree corresponding to the content being paid for.
    pub root_hash: Hash,
}

impl Default for FeeOutput {
    fn default() -> Self {
        Self {
            id: Hash::default(),
            token: Nano::zero(),
            root_hash: Hash::default(),
        }
    }
}

impl FeeOutput {
    pub fn new(id: Hash, amount: u64, root_hash: Hash) -> Self {
        Self {
            id,
            token: Nano::from(amount),
            root_hash,
        }
    }

    pub fn is_free(&self) -> bool {
        self.token.is_zero()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::<u8>::new();
        v.extend(self.id.slice());
        v.extend(self.token.to_bytes());
        v.extend(self.root_hash.slice());
        v
    }
}
