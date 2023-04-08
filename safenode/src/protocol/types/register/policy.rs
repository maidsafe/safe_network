// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    super::{
        authority::PublicKey,
        error::{Error, Result},
    },
    Action,
};

use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, hash::Hash};

/// Set of public permissions for a user.
#[derive(Copy, Clone, Serialize, Deserialize, PartialEq, PartialOrd, Ord, Eq, Hash, Debug)]
pub struct Permissions {
    /// `Some(true)` if the user can write.
    /// `Some(false)` explicitly denies this permission (even if `Anyone` has permissions).
    /// Use permissions for `Anyone` if `None`.
    write: Option<bool>,
}

impl Permissions {
    /// Constructs a new public permission set.
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
    Key(PublicKey),
}

/// Register permissions.
#[derive(Clone, Serialize, Deserialize, PartialEq, PartialOrd, Ord, Eq, Hash, Debug)]
pub struct Policy {
    /// An owner could represent an individual user, or a group of users,
    /// depending on the `public_key` type.
    pub owner: User,
    /// Map of users to their public permission set.
    pub permissions: BTreeMap<User, Permissions>,
}

impl Policy {
    /// Returns `Ok(())` if `action` is allowed for the provided user and `Err(AccessDenied)` if
    /// this action is not permitted.
    pub fn is_action_allowed(&self, requester: User, action: Action) -> Result<()> {
        // First checks if the requester is the owner.
        if action == Action::Read || requester == self.owner {
            Ok(())
        } else {
            match self
                .is_action_allowed_by_user(&requester, action)
                .or_else(|| self.is_action_allowed_by_user(&User::Anyone, action))
            {
                Some(true) => Ok(()),
                Some(false) => Err(Error::AccessDenied(requester)),
                None => Err(Error::AccessDenied(requester)),
            }
        }
    }

    /// Returns `Some(true)` if `action` is allowed for the provided user and `Some(false)` if it's
    /// not permitted. `None` means that default permissions should be applied.
    fn is_action_allowed_by_user(&self, user: &User, action: Action) -> Option<bool> {
        self.permissions
            .get(user)
            .and_then(|perms| perms.is_allowed(action))
    }

    /// Gets the permissions for a user if applicable.
    pub fn permissions(&self, user: User) -> Option<Permissions> {
        if user == self.owner {
            // i.e. it won't be possible to circumvent the semantics of `owner`
            // by setting some other permissions for the user.
            // the permissions can still be kept in the state though, so that switching owners gives an immediate permission update as well
            Some(Permissions::new(true))
        } else {
            self.permissions.get(&user).copied()
        }
    }

    /// Returns the owner.
    pub fn owner(&self) -> &User {
        &self.owner
    }
}
