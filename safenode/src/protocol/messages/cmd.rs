// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{super::types::chunk::Chunk, RegisterCmd};
use serde::{Deserialize, Serialize};

/// Data cmds - creating, updating, or removing data.
///
/// See the [`types`] module documentation for more details of the types supported by the Safe
/// Network, and their semantics.
///
/// [`types`]: crate::protocol::types
#[derive(Eq, PartialEq, Clone, Serialize, Deserialize, Debug)]
pub enum Cmd {
    /// [`Chunk`] write operation.
    ///
    /// [`Chunk`]: crate::protocol::types::chunk::Chunk
    StoreChunk(Chunk),
    /// [`Register`] write operation.
    ///
    /// [`Register`]: crate::protocol::types::register::Register
    Register(RegisterCmd),
}
