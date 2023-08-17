// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::error::Error;
use libp2p::kad::Record;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Debug, Serialize, Deserialize)]
pub struct RecordHeader {
    pub kind: RecordKind,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum RecordKind {
    Chunk,
    DbcSpend,
    Register,
}

impl Serialize for RecordKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match *self {
            Self::Chunk => serializer.serialize_u32(0),
            Self::DbcSpend => serializer.serialize_u32(1),
            Self::Register => serializer.serialize_u32(2),
        }
    }
}

impl<'de> Deserialize<'de> for RecordKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let num = u32::deserialize(deserializer)?;
        match num {
            0 => Ok(Self::Chunk),
            1 => Ok(Self::DbcSpend),
            2 => Ok(Self::Register),
            _ => Err(serde::de::Error::custom(
                "Unexpected integer for RecordKind variant",
            )),
        }
    }
}
impl Display for RecordKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RecordKind({self:?})")
    }
}

impl RecordHeader {
    pub const SIZE: usize = 2;

    pub fn try_serialize(self) -> Result<Vec<u8>, Error> {
        rmp_serde::to_vec(&self).map_err(|_| Error::RecordHeaderParsingFailed)
    }

    pub fn try_deserialize(bytes: &[u8]) -> Result<Self, Error> {
        rmp_serde::from_slice(bytes).map_err(|_| Error::RecordHeaderParsingFailed)
    }

    pub fn from_record(record: &Record) -> Result<Self, Error> {
        if record.value.len() <= RecordHeader::SIZE {
            return Err(Error::RecordHeaderParsingFailed);
        }
        Self::try_deserialize(&record.value[..RecordHeader::SIZE + 1])
            .map_err(|_| Error::RecordHeaderParsingFailed)
    }
}

/// Utility to deserialize a `KAD::Record` into any type.
/// Use `RecordHeader::from_record` if you want the `RecordHeader` instead.
pub fn try_deserialize_record<T: serde::de::DeserializeOwned>(record: &Record) -> Result<T, Error> {
    if record.value.len() <= RecordHeader::SIZE {
        return Err(Error::RecordParsingFailed);
    }
    let bytes = &record.value[RecordHeader::SIZE..];
    rmp_serde::from_slice(bytes).map_err(|_| Error::RecordParsingFailed)
}

/// Utility to serialize the provided data along with the RecordKind to be stored as Record::value
pub fn try_serialize_record<T: serde::Serialize>(
    data: &T,
    record_kind: RecordKind,
) -> Result<Vec<u8>, Error> {
    let payload = rmp_serde::to_vec(data).map_err(|_| Error::RecordParsingFailed)?;
    let mut record_value = RecordHeader { kind: record_kind }.try_serialize()?;
    record_value.extend(payload);

    Ok(record_value)
}

#[cfg(test)]
mod tests {
    use super::{RecordHeader, RecordKind};
    use crate::error::Result;

    #[test]
    fn verify_record_header_encoded_size() -> Result<()> {
        let chunk = RecordHeader {
            kind: RecordKind::Chunk,
        }
        .try_serialize()?;
        assert_eq!(chunk.len(), RecordHeader::SIZE);

        let spend = RecordHeader {
            kind: RecordKind::DbcSpend,
        }
        .try_serialize()?;
        assert_eq!(spend.len(), RecordHeader::SIZE);

        let register = RecordHeader {
            kind: RecordKind::Register,
        }
        .try_serialize()?;
        assert_eq!(register.len(), RecordHeader::SIZE);

        Ok(())
    }
}
