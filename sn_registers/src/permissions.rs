// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::Action;

use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, hash::Hash};

/// The rights of an user
#[derive(Copy, Clone, Serialize, Deserialize, PartialEq, PartialOrd, Ord, Eq, Hash, Debug)]
pub struct UserRights {
    /// `Some(true)` if the user can write.
    /// `Some(false)` explicitly denies writes (even if `Anyone` has the right to write).
    /// `None` use default: the rights for `Anyone`
    write: Option<bool>,
}

impl UserRights {
    /// Constructs a new set of user rights.
    pub fn new(write: impl Into<Option<bool>>) -> Self {
        Self {
            write: write.into(),
        }
    }

    /// Returns `Some(true)` if `action` is allowed and `Some(false)` if it's not permitted.
    /// `None` means that default permissions should be applied.
    pub fn is_allowed(self, action: Action) -> Option<bool> {
        match action {
            Action::Read => Some(true), // It's public data, so it's always allowed to read it.
            Action::Write => self.write,
        }
    }
}

/// User that can access a Register.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
pub enum User {
    /// Any user.
    Anyone,
    /// User identified by its public key.
    Key(bls::PublicKey),
}

/// Register permissions
/// Map of users to their public permission set.
pub type Permissions = BTreeMap<User, UserRights>;
