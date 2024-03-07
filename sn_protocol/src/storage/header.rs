// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::error::Error;
use crate::PrettyPrintRecordKey;
use bytes::{BufMut, Bytes, BytesMut};
use libp2p::kad::Record;
use rmp_serde::Serializer;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use xor_name::XorName;

/// Indicates the type of the record content.
/// Note for `Spend` and `Register`, using its content_hash (in `XorName` format)
/// to indicate different content body.
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Hash)]
pub enum RecordType {
    Chunk,
    NonChunk(XorName),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecordHeader {
    pub kind: RecordKind,
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum RecordKind {
    Chunk,
    ChunkWithPayment,
    Spend,
    Register,
    RegisterWithPayment,
}

impl Serialize for RecordKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match *self {
            Self::ChunkWithPayment => serializer.serialize_u32(0),
            Self::Chunk => serializer.serialize_u32(1),
            Self::Spend => serializer.serialize_u32(2),
            Self::Register => serializer.serialize_u32(3),
            Self::RegisterWithPayment => serializer.serialize_u32(4),
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
            0 => Ok(Self::ChunkWithPayment),
            1 => Ok(Self::Chunk),
            2 => Ok(Self::Spend),
            3 => Ok(Self::Register),
            4 => Ok(Self::RegisterWithPayment),
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

    pub fn try_serialize(self) -> Result<BytesMut, Error> {
        let bytes = BytesMut::new();
        let mut buf = bytes.writer();

        self.serialize(&mut Serializer::new(&mut buf))
            .map_err(|err| {
                error!("Failed to serialized RecordHeader {self:?} with error: {err:?}");
                Error::RecordHeaderParsingFailed
            })?;

        let b = buf.into_inner();

        Ok(b)
    }

    pub fn try_deserialize(bytes: &[u8]) -> Result<Self, Error> {
        rmp_serde::from_slice(bytes).map_err(|err| {
            error!("Failed to deserialize RecordHeader with error: {err:?}");
            Error::RecordHeaderParsingFailed
        })
    }

    pub fn from_record(record: &Record) -> Result<Self, Error> {
        if record.value.len() < RecordHeader::SIZE + 1 {
            return Err(Error::RecordHeaderParsingFailed);
        }
        Self::try_deserialize(&record.value[..RecordHeader::SIZE + 1])
    }

    pub fn is_record_of_type_chunk(record: &Record) -> Result<bool, Error> {
        let kind = Self::from_record(record)?.kind;
        Ok(kind == RecordKind::Chunk)
    }
}

/// Utility to deserialize a `KAD::Record` into any type.
/// Use `RecordHeader::from_record` if you want the `RecordHeader` instead.
pub fn try_deserialize_record<T: serde::de::DeserializeOwned>(record: &Record) -> Result<T, Error> {
    let bytes = if record.value.len() > RecordHeader::SIZE {
        &record.value[RecordHeader::SIZE..]
    } else {
        return Err(Error::RecordParsingFailed);
    };
    rmp_serde::from_slice(bytes).map_err(|err| {
        error!(
            "Failed to deserialized record {} with error: {err:?}",
            PrettyPrintRecordKey::from(&record.key)
        );
        Error::RecordParsingFailed
    })
}

/// Utility to serialize the provided data along with the RecordKind to be stored as Record::value
/// Returns Bytes to avoid accidental clone allocations
pub fn try_serialize_record<T: serde::Serialize>(
    data: &T,
    record_kind: RecordKind,
) -> Result<Bytes, Error> {
    let mut buf = RecordHeader { kind: record_kind }.try_serialize()?.writer();
    data.serialize(&mut Serializer::new(&mut buf))
        .map_err(|err| {
            error!("Failed to serialized Records with error: {err:?}");
            Error::RecordParsingFailed
        })?;
    let bytes = buf.into_inner();
    Ok(bytes.freeze())
}

#[cfg(test)]
mod tests {
    use super::{RecordHeader, RecordKind};
    use crate::error::Result;

    #[test]
    fn verify_record_header_encoded_size() -> Result<()> {
        let chunk_with_payment = RecordHeader {
            kind: RecordKind::ChunkWithPayment,
        }
        .try_serialize()?;
        assert_eq!(chunk_with_payment.len(), RecordHeader::SIZE);

        let reg_with_payment = RecordHeader {
            kind: RecordKind::RegisterWithPayment,
        }
        .try_serialize()?;
        assert_eq!(reg_with_payment.len(), RecordHeader::SIZE);

        let chunk = RecordHeader {
            kind: RecordKind::Chunk,
        }
        .try_serialize()?;
        assert_eq!(chunk.len(), RecordHeader::SIZE);

        let spend = RecordHeader {
            kind: RecordKind::Spend,
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
