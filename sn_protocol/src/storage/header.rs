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
use std::fmt::Display;

#[derive(Debug, Serialize, Deserialize)]
pub struct RecordHeader {
    pub kind: RecordKind,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub enum RecordKind {
    Chunk,
    DbcSpend,
    Register,
}

impl Display for RecordKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RecordKind({self:?})")
    }
}

impl RecordHeader {
    // Bincode serializes enums with unit variants as a u32, not a u8, so it would take up 4 bytes.
    pub const SIZE: usize = 4;

    pub fn from_record(record: &Record) -> Result<Self> {
        bincode::deserialize(&record.value[..RecordHeader::SIZE + 1])
            .map_err(|_| Error::RecordHeaderParsingFailed)
    }
}

pub fn try_deserialize_record<T: serde::de::DeserializeOwned>(record: &Record) -> Result<T> {
    let bytes = &record.value[RecordHeader::SIZE..];
    bincode::deserialize(bytes).map_err(|_| Error::RecordParsingFailed)
}

pub fn try_serialize_record<T: serde::Serialize>(
    data: &T,
    record_kind: RecordKind,
) -> Result<Vec<u8>> {
    let payload = bincode::serialize(data).map_err(|_| Error::RecordParsingFailed)?;
    let record_header = RecordHeader { kind: record_kind };

    let mut record_value =
        bincode::serialize(&record_header).map_err(|_| Error::RecordParsingFailed)?;

    record_value.extend(payload);
    Ok(record_value)
}
