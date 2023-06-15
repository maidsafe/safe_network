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
    pub const SIZE: usize = 7;

    pub fn try_serialize(self) -> Result<Vec<u8>> {
        rmp_serde::to_vec(&self).map_err(|_| Error::RecordHeaderParsingFailed)
    }

    pub fn try_deserialize(bytes: &[u8]) -> Result<Self> {
        rmp_serde::from_slice(bytes).map_err(|_| Error::RecordHeaderParsingFailed)
    }

    pub fn from_record(record: &Record) -> Result<Self> {
        Self::try_deserialize(&record.value[..RecordHeader::SIZE + 1])
            .map_err(|_| Error::RecordHeaderParsingFailed)
    }
}

/// Utility to deserialize a `KAD::Record` into any type.
/// Use `RecordHeader::from_record` if you want the `RecordHeader` instead.
pub fn try_deserialize_record<T: serde::de::DeserializeOwned>(record: &Record) -> Result<T> {
    let bytes = &record.value[RecordHeader::SIZE..];
    rmp_serde::from_slice(bytes).map_err(|_| Error::RecordParsingFailed)
}

/// Utility to serialize the provided data along with the RecordKind to be stored as Record::value
pub fn try_serialize_record<T: serde::Serialize>(
    data: &T,
    record_kind: RecordKind,
) -> Result<Vec<u8>> {
    let payload = rmp_serde::to_vec(data).map_err(|_| Error::RecordParsingFailed)?;
    let mut record_value = RecordHeader { kind: record_kind }.try_serialize()?;
    record_value.extend(payload);

    Ok(record_value)
}
