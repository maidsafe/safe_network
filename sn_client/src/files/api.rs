// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    chunks::{to_chunk, DataMapLevel, Error as ChunksError, SmallFile},
    error::Result,
    Client, WalletClient,
};
use bytes::Bytes;
use futures::{future::join_all, stream::FuturesOrdered, StreamExt};
use itertools::Itertools;
use libp2p::PeerId;
use self_encryption::{self, ChunkInfo, DataMap, EncryptedChunk, MIN_ENCRYPTABLE_BYTES};
use self_encryption::{decrypt_full_set, StreamSelfDecryptor};
use sn_protocol::{
    storage::{Chunk, ChunkAddress},
    NetworkAddress,
};
use sn_transfers::{LocalWallet, NanoTokens};

use std::{
    fs::{self, create_dir_all, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Instant,
};
use tempfile::tempdir;
use tokio::task;
use tracing::trace;
use xor_name::XorName;

/// File APIs.
#[derive(Clone)]
pub struct FilesApi {
    pub(crate) client: Client,
    pub(crate) wallet_dir: PathBuf,
}

/// This is the (file xorname, datamap_data, filesize, and chunks)
/// If the DataMapChunk exists and is not stored on the network, then it will not be accessible at this address of ChunkAddress(XorName) .
type ChunkFileResult = Result<(ChunkAddress, Option<Bytes>, u64, Vec<(XorName, PathBuf)>)>;

impl FilesApi {
    /// Create file apis instance.
    pub fn new(client: Client, wallet_dir: PathBuf) -> Self {
        Self { client, wallet_dir }
    }

    /// Return the client instance
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Create a new WalletClient for a given root directory.
    pub fn wallet(&self) -> Result<WalletClient> {
        let path = self.wallet_dir.as_path();
        let wallet = LocalWallet::load_from(path)?;

        Ok(WalletClient::new(self.client.clone(), wallet))
    }

    /// Reads a file from the network, whose contents are contained within one or more chunks.
    /// Optionally we can pass a data_map_chunk if the data_map is local to this machine
    pub async fn read_bytes(
        &self,
        address: ChunkAddress,
        downloaded_file_path: Option<PathBuf>,
        data_map_chunk: Option<Chunk>,
        show_holders: bool,
        batch_size: usize,
    ) -> Result<Option<Bytes>> {
        let chunk = if let Some(chunk) = data_map_chunk {
            info!("Downloading via supplied local datamap");
            chunk
        } else {
            match self.client.get_chunk(address, show_holders).await {
                Ok(chunk) => chunk,
                Err(err) => {
                    error!("Failed to fetch head chunk {address:?}");
                    return Err(err);
                }
            }
        };

        // first try to deserialize a LargeFile, if it works, we go and seek it
        if let Ok(data_map) = self.unpack_chunk(chunk.clone(), batch_size).await {
            self.read_all(data_map, downloaded_file_path, show_holders, batch_size)
                .await
        } else {
            // if an error occurs, we assume it's a SmallFile
            if let Some(path) = downloaded_file_path {
                fs::write(path, chunk.value().clone())?;
                Ok(None)
            } else {
                Ok(Some(chunk.value().clone()))
            }
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
    pub async fn read_from(
        &self,
        address: ChunkAddress,
        position: usize,
        length: usize,
        batch_size: usize,
    ) -> Result<Bytes> {
        trace!("Reading {length} bytes at: {address:?}, starting from position: {position}");
        let chunk = self.client.get_chunk(address, false).await?;

        // First try to deserialize a LargeFile, if it works, we go and seek it.
        // If an error occurs, we consider it to be a SmallFile.
        if let Ok(data_map) = self.unpack_chunk(chunk.clone(), batch_size).await {
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

    /// Tries to chunk the file, returning `(head_address, data_map_chunk, file_size, chunk_names)`
    /// and writes encrypted chunks to disk.
    pub fn chunk_file(
        file_path: &Path,
        chunk_dir: &Path,
        include_data_map_in_chunks: bool,
    ) -> ChunkFileResult {
        let mut file = File::open(file_path)?;
        let metadata = file.metadata()?;
        let file_size = metadata.len();

        let (head_address, data_map_chunk, mut chunks_paths) =
            if file_size < MIN_ENCRYPTABLE_BYTES as u64 {
                let mut bytes = Vec::new();
                let _ = file.read_to_end(&mut bytes)?;
                let chunk = package_small(SmallFile::new(bytes.into())?)?;

                // Write the result to disk
                let small_chunk_file_path = chunk_dir.join(hex::encode(*chunk.name()));
                info!("Creating normal small chunk in {small_chunk_file_path:?}");
                let mut output_file = File::create(small_chunk_file_path.clone())?;
                output_file.write_all(&chunk.value)?;

                (
                    *chunk.name(),
                    None,
                    vec![(*chunk.name(), small_chunk_file_path)],
                )
            } else {
                let (data_map_chunk, chunks) = encrypt_large(file_path, chunk_dir)?;
                (*data_map_chunk.name(), Some(data_map_chunk), chunks)
            };

        debug!("include_data_map_in_chunks {include_data_map_in_chunks:?}");

        debug!(
            "Is there a datamap for chuink?? {:?}",
            data_map_chunk.is_some()
        );
        // only write out the data_map if one exists for this file
        if let Some(data_map_chunk) = &data_map_chunk {
            if include_data_map_in_chunks {
                info!("Data_map_chunk to be written!");
                let data_map_path = chunk_dir.join(hex::encode(*data_map_chunk.name()));

                trace!("Data_map_chunk being written to {data_map_path:?}");
                let mut output_file = File::create(data_map_path.clone())?;
                output_file.write_all(&data_map_chunk.value)?;

                chunks_paths.push((*data_map_chunk.name(), data_map_path))
            }
        }

        Ok((
            ChunkAddress::new(head_address),
            data_map_chunk.map(|c| c.value),
            file_size,
            chunks_paths,
        ))
    }

    /// Directly writes Chunks to the network in the
    /// form of immutable self encrypted chunks.
    ///
    pub async fn get_local_payment_and_upload_chunk(
        &self,
        chunk: Chunk,
        payee: PeerId,
        verify_store: bool,
    ) -> Result<()> {
        let chunk_addr = chunk.network_address();
        trace!("Client upload started for chunk: {chunk_addr:?} to {payee:?}");

        let wallet_client = self.wallet()?;
        let payment = wallet_client.get_payment_for_addr(&chunk_addr)?;

        debug!(
            "{:?} payments for chunk: {chunk_addr:?}:  {payment:?}",
            payment
        );

        self.client
            .store_chunk(chunk, payee, payment, verify_store)
            .await?;

        trace!("Client upload completed for chunk: {chunk_addr:?}");
        Ok(())
    }

    /// Pay for a given set of chunks.
    ///
    /// Returns the cost and the resulting new balance of the local wallet.
    pub async fn pay_for_chunks(
        &self,
        chunks: Vec<XorName>,
    ) -> Result<(
        (NanoTokens, NanoTokens, NanoTokens),
        (Vec<(XorName, PeerId)>, Vec<XorName>),
    )> {
        let mut wallet_client = self.wallet()?;
        info!("Paying for and uploading {:?} chunks", chunks.len());

        let ((storage_cost, royalties_fees), (payee_map, skipped_chunks)) =
            wallet_client
                .pay_for_storage(chunks.iter().map(|name| {
                    sn_protocol::NetworkAddress::ChunkAddress(ChunkAddress::new(*name))
                }))
                .await?;

        wallet_client.store_local_wallet()?;
        let new_balance = wallet_client.balance();
        Ok((
            (storage_cost, royalties_fees, new_balance),
            (payee_map, skipped_chunks),
        ))
    }

    // --------------------------------------------
    // ---------- Private helpers -----------------
    // --------------------------------------------

    /// Used for testing
    pub async fn upload_test_bytes(&self, bytes: Bytes, verify: bool) -> Result<NetworkAddress> {
        let temp_dir = tempdir()?;
        let file_path = temp_dir.path().join("tempfile");
        let mut file = File::create(&file_path)?;
        file.write_all(&bytes)?;

        let chunk_path = temp_dir.path().join("chunk_path");
        create_dir_all(chunk_path.clone())?;

        let (head_address, _data_map, _file_size, chunks_paths) =
            Self::chunk_file(&file_path, &chunk_path, true)?;

        for (_chunk_name, chunk_path) in chunks_paths {
            let chunk = Chunk::new(Bytes::from(fs::read(chunk_path)?));
            self.get_local_payment_and_upload_chunk(chunk, PeerId::random(), verify)
                .await?;
        }

        Ok(NetworkAddress::ChunkAddress(head_address))
    }

    // Gets and decrypts chunks from the network using nothing else but the data map.
    // If a downloaded path is given, the decrypted file will be written to the given path,
    // by the decryptor directly.
    // Otherwise, will assume the fetched content is a small one and return as bytes.
    async fn read_all(
        &self,
        data_map: DataMap,
        decrypted_file_path: Option<PathBuf>,
        show_holders: bool,
        batch_size: usize,
    ) -> Result<Option<Bytes>> {
        let mut decryptor = if let Some(path) = decrypted_file_path {
            StreamSelfDecryptor::decrypt_to_file(Box::new(path), &data_map)?
        } else {
            let encrypted_chunks = self.try_get_chunks(data_map.infos()).await?;
            let bytes = decrypt_full_set(&data_map, &encrypted_chunks)
                .map_err(ChunksError::SelfEncryption)?;
            return Ok(Some(bytes));
        };

        let expected_count = data_map.infos().len();
        // let mut missing_chunks = Vec::new();
        let mut ordered_read_futures = FuturesOrdered::new();
        let now = Instant::now();

        let mut index = 0;

        for chunk_info in data_map.infos().iter() {
            let dst_hash = chunk_info.dst_hash;
            // The futures are executed concurrently,
            // but the result is returned in the order in which they were inserted.
            ordered_read_futures.push_back(async move {
                (
                    dst_hash,
                    self.client
                        .get_chunk(ChunkAddress::new(dst_hash), show_holders)
                        .await,
                )
            });

            if ordered_read_futures.len() >= batch_size || index + batch_size > expected_count {
                while let Some((dst_hash, result)) = ordered_read_futures.next().await {
                    let chunk = result.map_err(|error| {
                        error!("Chunk missing {dst_hash:?} with {error:?}");
                        ChunksError::ChunkMissing(dst_hash)
                    })?;
                    let encrypted_chunk = EncryptedChunk {
                        index,
                        content: chunk.value().clone(),
                    };
                    let _ = decryptor.next_encrypted(encrypted_chunk)?;

                    index += 1;
                    info!("Client (read all) download progress {index:?}/{expected_count:?}");
                    println!("Client (read all) download progress {index:?}/{expected_count:?}");
                }
            }
        }

        let elapsed = now.elapsed();
        println!("Client downloaded file in {elapsed:?}");

        Ok(None)
    }

    /// Extracts a file DataMapLevel from a chunk.
    /// If the DataMapLevel is not the first level mapping directly to the user's contents,
    /// the process repeats itself until it obtains the first level DataMapLevel.
    pub async fn unpack_chunk(&self, mut chunk: Chunk, batch_size: usize) -> Result<DataMap> {
        loop {
            match rmp_serde::from_slice(chunk.value()).map_err(ChunksError::Deserialisation)? {
                DataMapLevel::First(data_map) => {
                    return Ok(data_map);
                }
                DataMapLevel::Additional(data_map) => {
                    let serialized_chunk = self
                        .read_all(data_map, None, false, batch_size)
                        .await?
                        .expect("error encountered on reading additional datamap");
                    chunk = rmp_serde::from_slice(&serialized_chunk)
                        .map_err(ChunksError::Deserialisation)?;
                }
            }
        }
    }
    // Gets a subset of chunks from the network, decrypts and
    // reads `len` bytes of the data starting at given `pos` of original file.
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
                .map_err(ChunksError::SelfEncryption)?;

        Ok(bytes)
    }

    async fn try_get_chunks(&self, chunks_info: Vec<ChunkInfo>) -> Result<Vec<EncryptedChunk>> {
        let expected_count = chunks_info.len();
        let mut retrieved_chunks = vec![];

        let mut tasks = Vec::new();
        for chunk_info in chunks_info.clone().into_iter() {
            let client = self.client.clone();
            let task = task::spawn(async move {
                let chunk = client
                    .get_chunk(ChunkAddress::new(chunk_info.dst_hash), false)
                    .await
                    .map_err(|error| {
                        error!("Chunk missing {:?} with {error:?}", chunk_info.dst_hash);
                        ChunksError::ChunkMissing(chunk_info.dst_hash)
                    })?;
                Ok::<EncryptedChunk, ChunksError>(EncryptedChunk {
                    index: chunk_info.index,
                    content: chunk.value().clone(),
                })
            });
            tasks.push(task);
        }

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
            Err(ChunksError::NotEnoughChunksRetrieved {
                expected: expected_count,
                retrieved: retrieved_chunks.len(),
                missing_chunks,
            })?
        } else {
            Ok(retrieved_chunks)
        }
    }
}

/// Encrypts a [`LargeFile`] and returns the resulting address and all chunk names.
/// Correspondent encrypted chunks are written in the specified output folder.
/// Does not store anything to the network.
///
/// Returns data map as a chunk, and the resulting chunks
fn encrypt_large(file_path: &Path, output_dir: &Path) -> Result<(Chunk, Vec<(XorName, PathBuf)>)> {
    Ok(crate::chunks::encrypt_large(file_path, output_dir)?)
}

/// Packages a [`SmallFile`] and returns the resulting address and the chunk.
/// Does not store anything to the network.
fn package_small(file: SmallFile) -> Result<Chunk> {
    let chunk = to_chunk(file.bytes());
    if chunk.value().len() >= self_encryption::MIN_ENCRYPTABLE_BYTES {
        return Err(ChunksError::SmallFilePaddingNeeded(chunk.value().len()).into());
    }
    Ok(chunk)
}
