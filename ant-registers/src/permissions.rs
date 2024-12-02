// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use bls::PublicKey;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, hash::Hash};

/// Register permissions
/// Everyone can read a Register, all data is public on safe network.
/// The Default value is nobody can write.
#[derive(Clone, Serialize, Deserialize, PartialEq, PartialOrd, Ord, Eq, Hash, Debug)]
pub enum Permissions {
    /// Anyone can write to this Register
    AnyoneCanWrite,
    /// This is the list of users allowed to write to this Register
    Writers(BTreeSet<PublicKey>),
}

impl Default for Permissions {
    fn default() -> Permissions {
        Permissions::Writers(BTreeSet::default())
    }
}

impl Permissions {
    /// Constructs a new set of permissions with a list of users allowed to write
    /// Empty list means nobody can write
    pub fn new_with(writers: impl IntoIterator<Item = PublicKey>) -> Self {
        Self::Writers(writers.into_iter().collect())
    }

    /// Constructs a new set of permissions where everyone can write
    pub fn new_anyone_can_write() -> Self {
        Self::AnyoneCanWrite
    }

    /// Checks is everyone can write to this Register
    pub fn can_anyone_write(&self) -> bool {
        matches!(self, Self::AnyoneCanWrite)
    }

    /// Returns true if the given user can write to this Register
    pub fn can_write(&self, user: &PublicKey) -> bool {
        match self {
            Self::AnyoneCanWrite => true,
            Self::Writers(writers) => writers.contains(user),
        }
    }

    /// If this is restricted to a set of users, add a user to the list of users that can write to this Register
    pub fn add_writer(&mut self, user: PublicKey) {
        if let Self::Writers(writers) = self {
            writers.insert(user);
        }
    }
}
