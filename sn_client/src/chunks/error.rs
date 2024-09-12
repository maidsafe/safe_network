// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use self_encryption::MIN_ENCRYPTABLE_BYTES;
use sn_protocol::PrettyPrintRecordKey;
use std::io;
use thiserror::Error;
use xor_name::XorName;

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

/// Internal error.
#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to get find payment for record: {0:?}")]
    NoPaymentForRecord(PrettyPrintRecordKey<'static>),

    #[error("Failed to get chunk permit")]
    CouldNotGetChunkPermit,

    #[error(transparent)]
    SelfEncryption(#[from] self_encryption::Error),

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Serialisation(#[from] rmp_serde::encode::Error),

    #[error(transparent)]
    Deserialisation(#[from] rmp_serde::decode::Error),

    #[error("Cannot store empty file.")]
    EmptyFileProvided,

    #[error("File is too small to be encrypted, it is less than {MIN_ENCRYPTABLE_BYTES} bytes")]
    FileTooSmall,

    #[error(
        "The provided bytes ({size}) is too large to store as a `SmallFile` which maximum can be \
        {maximum}. Store as a LargeFile instead."
    )]
    TooLargeAsSmallFile {
        /// Number of bytes
        size: usize,
        /// Maximum number of bytes for a `SmallFile`
        maximum: usize,
    },

    #[error("Not all chunks were retrieved, expected {expected}, retrieved {retrieved}, missing {missing_chunks:?}.")]
    NotEnoughChunksRetrieved {
        /// Number of Chunks expected to be retrieved
        expected: usize,
        /// Number of Chunks retrieved
        retrieved: usize,
        /// Missing chunks
        missing_chunks: Vec<XorName>,
    },

    #[error("Chunk could not be retrieved from the network: {0:?}")]
    ChunkMissing(XorName),

    #[error("Not all data was chunked, expected {expected}, but we have {chunked}.)")]
    NotAllDataWasChunked {
        /// Number of Chunks expected to be generated
        expected: usize,
        /// Number of Chunks generated
        chunked: usize,
    },
}
