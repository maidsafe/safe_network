// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::protocol::types::{address::RegisterAddress, errors::Error as ProtocolError};

use bls::PublicKey;
use std::io;
use thiserror::Error;
use xor_name::XorName;

/// Specialisation of `std::Result` for storage mod.
pub(super) type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Error, Debug)]
#[non_exhaustive]
/// Node error variants.
pub enum Error {
    /// Not enough space to store the value.
    #[error("Not enough space")]
    NotEnoughSpace,
    /// Chunk not found.
    #[error("Chunk not found: {0:?}")]
    ChunkNotFound(XorName),
    /// Register not found.
    #[error("Register not found: {0:?}")]
    RegisterNotFound(RegisterAddress),
    /// NetworkData error.
    #[error("Network data error:: {0}")]
    NetworkData(#[from] ProtocolError),
    /// Data authority provided is invalid.
    #[error("Provided PublicKey could not validate signature {0:?}")]
    InvalidSignature(PublicKey),
    /// Bincode error.
    #[error("Bincode error:: {0}")]
    Bincode(#[from] bincode::Error),
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    /// Hex decoding error.
    #[error("Hex decoding error:: {0}")]
    HexDecoding(#[from] hex::FromHexError),
    /// Register command/op destination adddress mistmatch
    #[error(
        "Register command destination address ({cmd_dst_addr:?}) \
        doesn't match stored Register address: {reg_addr:?}"
    )]
    RegisterAddrMismatch {
        cmd_dst_addr: RegisterAddress,
        reg_addr: RegisterAddress,
    },
}

// Convert storage error to messaging error message for sending over the network.
impl From<Error> for ProtocolError {
    fn from(error: Error) -> ProtocolError {
        match error {
            Error::NotEnoughSpace => ProtocolError::FailedToWriteFile,
            Error::RegisterNotFound(address) => ProtocolError::RegisterNotFound(address),
            Error::ChunkNotFound(xorname) => ProtocolError::ChunkNotFound(xorname),
            Error::NetworkData(error) => error,
            Error::InvalidSignature(pk) => ProtocolError::InvalidSignature(pk),
            other => {
                ProtocolError::InvalidOperation(format!("Failed to perform operation: {other:?}"))
            }
        }
    }
}
