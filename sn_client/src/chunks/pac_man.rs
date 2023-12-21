// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::Result;
use bytes::{BufMut, Bytes, BytesMut};
use rayon::prelude::*;
use self_encryption::{DataMap, StreamSelfEncryptor, MAX_CHUNK_SIZE};
use serde::{Deserialize, Serialize};
use sn_protocol::storage::Chunk;
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};
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
pub(crate) fn encrypt_from_path(path: &Path, output_dir: &Path) -> Result<(Chunk, Vec<XorName>)> {
    let (data_map, mut encrypted_chunks) = encrypt_file(path, output_dir)?;

    let (data_map_chunk, additional_chunks) = pack_data_map(data_map)?;

    for chunk in additional_chunks.iter() {
        encrypted_chunks.push(*chunk.name());
        let file_path = output_dir.join(&hex::encode(chunk.name()));
        let mut output_file = File::create(file_path)?;
        output_file.write_all(&chunk.value)?;
    }

    Ok((data_map_chunk, encrypted_chunks))
}

#[allow(unused_assignments)]
pub(crate) fn encrypt_large(
    file_path: &Path,
    output_dir: &Path,
) -> Result<(Chunk, Vec<(XorName, PathBuf)>)> {
    let mut encryptor = StreamSelfEncryptor::encrypt_from_file(
        Box::new(file_path.to_path_buf()),
        Some(Box::new(output_dir.to_path_buf())),
    )?;

    let mut data_map = None;
    loop {
        match encryptor.next_encryption()? {
            (None, Some(m)) => {
                // Returning a data_map means file encryption is completed.
                data_map = Some(m);
                break;
            }
            _ => continue,
        }
    }
    let data_map = data_map.unwrap();
    let mut encrypted_chunks: Vec<_> = data_map
        .infos()
        .iter()
        .map(|chunk_info| {
            let chunk_file_path = output_dir.join(hex::encode(chunk_info.dst_hash));
            (chunk_info.dst_hash, chunk_file_path)
        })
        .collect();

    // Pack the datamap into chunks that under the same output folder as well.
    let (data_map_chunk, additional_chunks) = pack_data_map(data_map)?;
    for chunk in additional_chunks.iter() {
        let file_path = output_dir.join(&hex::encode(chunk.name()));
        encrypted_chunks.push((*chunk.name(), file_path.to_path_buf()));
        let mut output_file = File::create(file_path)?;
        output_file.write_all(&chunk.value)?;
    }

    Ok((data_map_chunk, encrypted_chunks))
}

pub(crate) fn to_chunk(chunk_content: Bytes) -> Chunk {
    Chunk::new(chunk_content)
}

// Produces a chunk out of the first `DataMap`, which is validated for its size.
// If the chunk is too big, it is self-encrypted and the resulting (additional level) `DataMap` is put into a chunk.
// The above step is repeated as many times as required until the chunk size is valid.
// In other words: If the chunk content is too big, it will be
// self encrypted into additional chunks, and now we have a new `DataMap`
// which points to all of those additional chunks.. and so on.
fn pack_data_map(data_map: DataMap) -> Result<(Chunk, Vec<Chunk>)> {
    let mut chunks = vec![];
    let mut chunk_content = wrap_data_map(&DataMapLevel::First(data_map))?;

    let (data_map_chunk, additional_chunks) = loop {
        let chunk = to_chunk(chunk_content);
        // If datamap chunk is less than `MAX_CHUNK_SIZE` return it so it can be directly sent to the network.
        if MAX_CHUNK_SIZE >= chunk.serialised_size() {
            chunks.reverse();
            // Returns the address of the last datamap, the datamap chunk, and all the chunks produced.
            break (chunk, chunks);
        } else {
            let mut bytes = BytesMut::with_capacity(MAX_CHUNK_SIZE).writer();
            let mut serialiser = rmp_serde::Serializer::new(&mut bytes);
            chunk.serialize(&mut serialiser)?;
            let serialized_chunk = bytes.into_inner().freeze();

            let (data_map, next_encrypted_chunks) = self_encryption::encrypt(serialized_chunk)?;
            chunks = next_encrypted_chunks
                .par_iter()
                .map(|c| to_chunk(c.content.clone())) // no need to encrypt what is self-encrypted
                .chain(chunks)
                .collect();
            chunk_content = wrap_data_map(&DataMapLevel::Additional(data_map))?;
        }
    };

    Ok((data_map_chunk, additional_chunks))
}

fn wrap_data_map(data_map: &DataMapLevel) -> Result<Bytes> {
    // we use an initial/starting size of 300 bytes as that's roughly the current size of a DataMapLevel instance.
    let mut bytes = BytesMut::with_capacity(300).writer();
    let mut serialiser = rmp_serde::Serializer::new(&mut bytes);
    data_map.serialize(&mut serialiser)?;
    Ok(bytes.into_inner().freeze())
}

fn encrypt_file(file: &Path, output_dir: &Path) -> Result<(DataMap, Vec<XorName>)> {
    let encrypted_chunks = self_encryption::encrypt_from_file(file, output_dir)?;
    Ok(encrypted_chunks)
}
