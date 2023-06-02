// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::error::{Error, Result};
use libp2p::kad::Record;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RecordHeader {
    pub kind: RecordKind,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RecordKind {
    Chunk,
    DbcSpend,
    Register,
}

impl RecordHeader {
    // Bincode serializes enums with unit variants as a u32, not a u8, so it would take up 4 bytes.
    pub const SIZE: usize = 4;

    pub fn from_record(record: &Record) -> Result<Self> {
        bincode::deserialize(&record.value[..RecordHeader::SIZE + 1])
            .map_err(|_| Error::RecordHeaderParsingFailed)
    }
}
