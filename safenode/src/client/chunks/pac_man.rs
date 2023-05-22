// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{Error, Result};

use bincode::serialize;
use bytes::Bytes;
use rayon::prelude::*;
use self_encryption::{DataMap, EncryptedChunk, MAX_CHUNK_SIZE};
use serde::{Deserialize, Serialize};
use sn_protocol::storage::Chunk;
use std::path::Path;
use xor_name::XorName;

#[derive(Serialize, Deserialize)]
pub(crate) enum DataMapLevel {
    // Holds the data map to the source data.
    First(DataMap),
    // Holds the data map of an _additional_ level of chunks
    // resulting from chunking up a previous level data map.
    // This happens when that previous level data map was too big to fit in a chunk itself.
    Additional(DataMap),
}

#[allow(unused)]
pub(crate) fn encrypt_from_path(path: &Path) -> Result<(XorName, Vec<Chunk>)> {
    let (data_map, encrypted_chunks) = encrypt_file(path)?;
    pack(data_map, encrypted_chunks)
}

pub(crate) fn encrypt_large(data: Bytes) -> Result<(XorName, Vec<Chunk>)> {
    let (data_map, encrypted_chunks) = encrypt_data(data)?;
    pack(data_map, encrypted_chunks)
}

/// Returns the top-most chunk address through which the entire
/// data tree can be accessed, and all the other encrypted chunks.
/// If encryption is provided, the additional `DataMapLevel` chunks are encrypted with it.
/// This is necessary if the data is meant to be private, since a `DataMap` is used to find and decrypt the original file.
pub(crate) fn pack(
    data_map: DataMap,
    encrypted_chunks: Vec<EncryptedChunk>,
) -> Result<(XorName, Vec<Chunk>)> {
    // Produces a chunk out of the first `DataMap`, which is validated for its size.
    // If the chunk is too big, it is self-encrypted and the resulting (additional level) `DataMap` is put into a chunk.
    // The above step is repeated as many times as required until the chunk size is valid.
    // In other words: If the chunk content is too big, it will be
    // self encrypted into additional chunks, and now we have a new `DataMap`
    // which points to all of those additional chunks.. and so on.
    let mut chunks = vec![];
    let mut chunk_content = pack_data_map(DataMapLevel::First(data_map))?;

    let (address, additional_chunks) = loop {
        let chunk = to_chunk(chunk_content);
        // If datamap chunk is less than `MAX_CHUNK_SIZE` return it so it can be directly sent to the network.
        if MAX_CHUNK_SIZE >= chunk.serialised_size() {
            let name = *chunk.name();
            chunks.reverse();
            chunks.push(chunk);
            // Returns the address of the last datamap, and all the chunks produced.
            break (name, chunks);
        } else {
            let serialized_chunk = Bytes::from(serialize(&chunk)?);
            let (data_map, next_encrypted_chunks) = self_encryption::encrypt(serialized_chunk)?;
            chunks = next_encrypted_chunks
                .par_iter()
                .map(|c| to_chunk(c.content.clone())) // no need to encrypt what is self-encrypted
                .chain(chunks)
                .collect();
            chunk_content = pack_data_map(DataMapLevel::Additional(data_map))?;
        }
    };

    let expected_total = encrypted_chunks.len() + additional_chunks.len();
    let all_chunks: Vec<_> = encrypted_chunks
        .par_iter()
        .map(|c| to_chunk(c.content.clone())) // no need to encrypt what is self-encrypted
        .chain(additional_chunks)
        .collect();

    if expected_total > all_chunks.len() {
        // as we flatten above, we need to check outcome here
        return Err(Error::NotAllDataWasChunked {
            expected: expected_total,
            chunked: all_chunks.len(),
        });
    }

    Ok((address, all_chunks))
}

pub(crate) fn to_chunk(chunk_content: Bytes) -> Chunk {
    Chunk::new(chunk_content)
}

fn pack_data_map(data_map: DataMapLevel) -> Result<Bytes> {
    Ok(Bytes::from(serialize(&data_map)?))
}

fn encrypt_file(file: &Path) -> Result<(DataMap, Vec<EncryptedChunk>)> {
    let bytes = Bytes::from(std::fs::read(file)?);
    let encrypted_chunk = self_encryption::encrypt(bytes)?;
    Ok(encrypted_chunk)
}

fn encrypt_data(bytes: Bytes) -> Result<(DataMap, Vec<EncryptedChunk>)> {
    let encrypted_chunk = self_encryption::encrypt(bytes)?;
    Ok(encrypted_chunk)
}
