// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    chunks::{to_chunk, DataMapLevel, Error, SmallFile},
    error::Result,
    Client,
};

use sn_protocol::{
    storage::{Chunk, ChunkAddress},
    NetworkAddress,
};
use sn_transfers::client_transfers::ContentPaymentsMap;

use bincode::deserialize;
use bytes::Bytes;
use futures::future::join_all;
use itertools::Itertools;
use self_encryption::{self, ChunkInfo, DataMap, EncryptedChunk, MIN_ENCRYPTABLE_BYTES};
use tokio::task;
use tracing::trace;
use xor_name::XorName;

// Maximum number of concurrent chunks to be uploaded/retrieved for a file
const CHUNKS_BATCH_MAX_SIZE: usize = 10;

// Maximum number of concurrent chunks to be uploaded at any one time, managed by a semaphore
pub const MAX_CONCURRENT_CHUNK_UPLOAD: usize = 1;

/// File APIs.
pub struct Files {
    client: Client,
}

impl Files {
    /// Create file apis instance.
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    #[instrument(skip(self), level = "debug")]
    /// Reads [`Bytes`] from the network, whose contents are contained within one or more chunks.
    pub async fn read_bytes(&self, address: ChunkAddress) -> Result<Bytes> {
        let chunk = self.client.get_chunk(address).await?;

        // first try to deserialize a LargeFile, if it works, we go and seek it
        if let Ok(data_map) = self.unpack_chunk(chunk.clone()).await {
            self.read_all(data_map).await
        } else {
            // if an error occurs, we assume it's a SmallFile
            Ok(chunk.value().clone())
        }
    }

    /// Read bytes from the network. The contents are spread across
    /// multiple chunks in the network. This function invokes the self-encryptor and returns
    /// the data that was initially stored.
    ///
    /// Takes `position` and `length` arguments which specify the start position
    /// and the length of bytes to be read.
    /// Passing `0` to position reads the data from the beginning,
    /// and the `length` is just an upper limit.
    #[instrument(skip_all, level = "trace")]
    pub async fn read_from(
        &self,
        address: ChunkAddress,
        position: usize,
        length: usize,
    ) -> Result<Bytes>
    where
        Self: Sized,
    {
        trace!("Reading {length} bytes at: {address:?}, starting from position: {position}");
        let chunk = self.client.get_chunk(address).await?;

        // First try to deserialize a LargeFile, if it works, we go and seek it.
        // If an error occurs, we consider it to be a SmallFile.
        if let Ok(data_map) = self.unpack_chunk(chunk.clone()).await {
            return self.seek(data_map, position, length).await;
        }

        // The error above is ignored to avoid leaking the storage format detail of SmallFiles and LargeFiles.
        // The basic idea is that we're trying to deserialize as one, and then the other.
        // The cost of it is that some errors will not be seen without a refactor.
        let mut bytes = chunk.value().clone();

        let _ = bytes.split_to(position);
        bytes.truncate(length);

        Ok(bytes)
    }

    /// Directly writes [`Bytes`] to the network in the
    /// form of immutable chunks, without any batching.
    #[instrument(skip(self, bytes), level = "debug")]
    pub async fn upload_with_payments(
        &self,
        bytes: Bytes,
        content_payments_map: ContentPaymentsMap,
        verify_store: bool,
    ) -> Result<NetworkAddress> {
        self.upload_bytes(bytes, content_payments_map, verify_store)
            .await
    }

    /// Calculates a LargeFile's/SmallFile's address from self encrypted chunks,
    /// without storing them onto the network.
    #[instrument(skip_all, level = "debug")]
    pub fn calculate_address(&self, bytes: Bytes) -> Result<XorName> {
        self.chunk_bytes(bytes).map(|(name, _)| name)
    }

    /// Tries to chunk the bytes, returning the data-map address and chunks,
    /// without storing anything to network.
    #[instrument(skip_all, level = "trace")]
    pub fn chunk_bytes(&self, bytes: Bytes) -> Result<(XorName, Vec<Chunk>)> {
        if bytes.len() < MIN_ENCRYPTABLE_BYTES {
            let file = SmallFile::new(bytes)?;
            let chunk = package_small(file)?;
            Ok((*chunk.name(), vec![chunk]))
        } else {
            encrypt_large(bytes)
        }
    }

    /// Directly writes Chunks to the network in the
    /// form of immutable self encrypted chunks.
    ///
    /// Each chunk should be accompanied by a semaphore permit, which will be released
    #[instrument(skip_all, level = "trace")]
    pub async fn upload_chunk_in_parallel(
        &self,
        chunk: Chunk,
        content_payments_map: &mut ContentPaymentsMap,
        verify_store: bool,
    ) -> Result<()> {
        let client = self.client.clone();
        let chunk_addr = chunk.network_address();
        trace!("Client upload started for chunk: {chunk_addr:?}");
        let payment = content_payments_map
            .remove(&chunk_addr)
            .ok_or(super::Error::MissingPaymentProof(format!("{chunk_addr}")))?;

        trace!(
            "Payment for {chunk_addr:?}: has length: {:?}",
            payment.len()
        );
        client.store_chunk(chunk, payment, verify_store).await?;

        trace!("Client upload completed for chunk: {chunk_addr:?}");
        Ok(())
    }

    // --------------------------------------------
    // ---------- Private helpers -----------------
    // --------------------------------------------

    /// Used for testing
    #[instrument(skip(self, bytes), level = "trace")]
    async fn upload_bytes(
        &self,
        bytes: Bytes,
        mut content_payments_map: ContentPaymentsMap,
        verify: bool,
    ) -> Result<NetworkAddress> {
        if bytes.len() < MIN_ENCRYPTABLE_BYTES {
            let file = SmallFile::new(bytes)?;
            self.upload_small(file, content_payments_map, verify).await
        } else {
            let (head_address, chunks) = encrypt_large(bytes)?;

            for chunk in chunks {
                self.upload_chunk_in_parallel(chunk, &mut content_payments_map, verify)
                    .await?;
            }

            Ok(NetworkAddress::ChunkAddress(ChunkAddress::new(
                head_address,
            )))
        }
    }

    /// Directly writes a [`SmallFile`] to the network in the
    /// form of a single chunk, without any batching.
    #[instrument(skip_all, level = "trace")]
    async fn upload_small(
        &self,
        small: SmallFile,
        content_payments_map: ContentPaymentsMap,
        verify_store: bool,
    ) -> Result<NetworkAddress> {
        let chunk = package_small(small)?;
        let address = chunk.network_address();
        let payment = content_payments_map
            .get(&address)
            .cloned()
            .ok_or(super::Error::MissingPaymentProof(format!("{address}")))?;

        self.client
            .store_chunk(chunk, payment, verify_store)
            .await?;

        Ok(address)
    }

    // Gets and decrypts chunks from the network using nothing else but the data map,
    // then returns the raw data.
    async fn read_all(&self, data_map: DataMap) -> Result<Bytes> {
        let encrypted_chunks = self.try_get_chunks(data_map.infos()).await?;
        let bytes = self_encryption::decrypt_full_set(&data_map, &encrypted_chunks)
            .map_err(Error::SelfEncryption)?;
        Ok(bytes)
    }

    /// Extracts a file DataMapLevel from a chunk.
    /// If the DataMapLevel is not the first level mapping directly to the user's contents,
    /// the process repeats itself until it obtains the first level DataMapLevel.
    #[instrument(skip_all, level = "trace")]
    async fn unpack_chunk(&self, mut chunk: Chunk) -> Result<DataMap> {
        loop {
            match deserialize(chunk.value()).map_err(Error::Serialisation)? {
                DataMapLevel::First(data_map) => {
                    return Ok(data_map);
                }
                DataMapLevel::Additional(data_map) => {
                    let serialized_chunk = self.read_all(data_map).await?;
                    chunk = deserialize(&serialized_chunk).map_err(Error::Serialisation)?;
                }
            }
        }
    }
    // Gets a subset of chunks from the network, decrypts and
    // reads `len` bytes of the data starting at given `pos` of original file.
    #[instrument(skip_all, level = "trace")]
    async fn seek(&self, data_map: DataMap, pos: usize, len: usize) -> Result<Bytes> {
        let info = self_encryption::seek_info(data_map.file_size(), pos, len);
        let range = &info.index_range;
        let all_infos = data_map.infos();

        let encrypted_chunks = self
            .try_get_chunks(
                (range.start..range.end + 1)
                    .clone()
                    .map(|i| all_infos[i].clone())
                    .collect_vec(),
            )
            .await?;

        let bytes =
            self_encryption::decrypt_range(&data_map, &encrypted_chunks, info.relative_pos, len)
                .map_err(Error::SelfEncryption)?;

        Ok(bytes)
    }

    #[instrument(skip_all, level = "trace")]
    async fn try_get_chunks(&self, chunks_info: Vec<ChunkInfo>) -> Result<Vec<EncryptedChunk>> {
        let expected_count = chunks_info.len();
        let mut retrieved_chunks = vec![];
        for next_batch in chunks_info.chunks(CHUNKS_BATCH_MAX_SIZE) {
            let tasks = next_batch.iter().cloned().map(|chunk_info| {
                let client = self.client.clone();
                task::spawn(async move {
                    match client
                        .get_chunk(ChunkAddress::new(chunk_info.dst_hash))
                        .await
                    {
                        Ok(chunk) => Ok(EncryptedChunk {
                            index: chunk_info.index,
                            content: chunk.value().clone(),
                        }),
                        Err(err) => {
                            warn!(
                                "Reading chunk {} from network, resulted in error {err:?}.",
                                chunk_info.dst_hash
                            );
                            Err(err)
                        }
                    }
                })
            });

            // This swallowing of errors is basically a compaction into a single
            // error saying "didn't get all chunks".
            retrieved_chunks.extend(join_all(tasks).await.into_iter().flatten().flatten());

            info!(
                "Client download progress {:?}/{expected_count:?}",
                retrieved_chunks.len()
            );
            println!(
                "Client download progress {:?}/{expected_count:?}",
                retrieved_chunks.len()
            );
        }

        if expected_count > retrieved_chunks.len() {
            let missing_chunks: Vec<XorName> = chunks_info
                .iter()
                .filter_map(|expected_info| {
                    if retrieved_chunks.iter().any(|retrieved_chunk| {
                        XorName::from_content(&retrieved_chunk.content) == expected_info.dst_hash
                    }) {
                        None
                    } else {
                        Some(expected_info.dst_hash)
                    }
                })
                .collect();
            Err(Error::NotEnoughChunksRetrieved {
                expected: expected_count,
                retrieved: retrieved_chunks.len(),
                missing_chunks,
            })?
        } else {
            Ok(retrieved_chunks)
        }
    }
}

/// Encrypts a [`LargeFile`] and returns the resulting address and all chunks.
/// Does not store anything to the network.
#[instrument(skip(bytes), level = "trace")]
fn encrypt_large(bytes: Bytes) -> Result<(XorName, Vec<Chunk>)> {
    Ok(super::chunks::encrypt_large(bytes)?)
}

/// Packages a [`SmallFile`] and returns the resulting address and the chunk.
/// Does not store anything to the network.
fn package_small(file: SmallFile) -> Result<Chunk> {
    let chunk = to_chunk(file.bytes());
    if chunk.value().len() >= self_encryption::MIN_ENCRYPTABLE_BYTES {
        return Err(Error::SmallFilePaddingNeeded(chunk.value().len()))?;
    }
    Ok(chunk)
}
