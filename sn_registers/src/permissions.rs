// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, hash::Hash};

/// User that can access a Register.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
pub enum User {
    /// Any user.
    Anyone,
    /// User identified by its public key.
    Key(bls::PublicKey),
}

/// Register permissions
/// Everyone can read a Register, all data is public on safe network.
/// The Default value is nobody can write.
#[derive(Clone, Serialize, Deserialize, PartialEq, PartialOrd, Ord, Eq, Hash, Debug, Default)]
pub struct Permissions {
    /// Anyone can read a Register since all data is public
    /// The owner of a register can always write to it
    /// This is the list of users that the owner has allowed to write to this Register
    pub writers: BTreeSet<User>,
}

impl Permissions {
    /// Constructs a new set of permissions with a list of users allowed to write
    /// Empty list means nobody (appart from the owner) can write
    pub fn new_with(writers: impl IntoIterator<Item = User>) -> Self {
        Self {
            writers: writers.into_iter().collect(),
        }
    }

    /// Constructs a new set of permissions where everyone can write
    pub fn new_anyone_can_write() -> Self {
        Self {
            writers: vec![User::Anyone].into_iter().collect(),
        }
    }

    /// Constructs a new set of permissions where only the owner can write
    pub fn new_owner_only() -> Self {
        Default::default()
    }

    /// Checks is everyone can write to this Register
    pub fn everyone_can_write(&self) -> bool {
        self.writers.contains(&User::Anyone)
    }

    /// Returns true if the given user can write to this Register
    pub fn can_write(&self, user: &User) -> bool {
        self.everyone_can_write() || self.writers.contains(user)
    }
}
