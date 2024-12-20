// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod chunk;
mod scratchpad;
mod linked_list;

pub use self::chunk::ChunkAddress;
pub use self::scratchpad::ScratchpadAddress;
pub use self::linked_list::LinkedListAddress;
pub use ant_registers::RegisterAddress;
