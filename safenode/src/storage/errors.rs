// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::{io, path::PathBuf};
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
    // /// Storage not supported for type of data address
    // #[error("Storage not supported for type of data address: {0:?}")]
    // UnsupportedDataType(DataAddress),
    // /// Data owner provided is invalid.
    // #[error("Provided PublicKey could not validate signature {0:?}")]
    // InvalidSignature(Box<PublicKey>),
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    /// Bincode error.
    // #[error("Bincode error:: {0}")]
    // Bincode(#[from] bincode::Error),
    // /// Hex decoding error.
    #[error("Hex decoding error:: {0}")]
    HexDecoding(#[from] hex::FromHexError),
    // /// NetworkData error.
    // #[error("Network data error:: {0}")]
    // NetworkData(#[from] sn_interface::types::Error),
    // /// Messaging error.
    // #[error("Messaging error:: {0}")]
    // Messaging(#[from] Box<sn_interface::messaging::data::Error>),
    /// No filename found
    #[error("Path contains no file name: {0}")]
    NoFilename(PathBuf),
    /// Invalid filename
    #[error("Invalid chunk filename: {0}")]
    InvalidFilename(PathBuf),
    // /// Register command/op destinaation adddress mistmatch
    // #[error(
    //     "Register command destination address ({cmd_dst_addr:?}) doesn't match stored Register address: {reg_addr:?}"
    // )]
    // RegisterAddrMismatch {
    //     cmd_dst_addr: RegisterAddress,
    //     reg_addr: RegisterAddress,
    // },
}
