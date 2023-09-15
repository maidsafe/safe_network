// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::path::PathBuf;

use crate::WalletClient;

use super::{
    chunks::{to_chunk, DataMapLevel, Error, SmallFile},
    error::Result,
    Client,
};

use self_encryption::{decrypt_full_set, StreamSelfDecryptor};
use sn_dbc::Token;
use sn_protocol::{
    storage::{Chunk, ChunkAddress},
    NetworkAddress, PrettyPrintRecordKey,
};
use sn_transfers::wallet::LocalWallet;

use bincode::deserialize;
use bytes::Bytes;
use futures::future::join_all;
use itertools::Itertools;
use self_encryption::{self, ChunkInfo, DataMap, EncryptedChunk, MIN_ENCRYPTABLE_BYTES};
use std::{
    fs::{self, create_dir_all, File},
    io::{Read, Write},
    path::Path,
};
use tempfile::tempdir;
use tokio::{sync::OwnedSemaphorePermit, task};
use tracing::trace;
use xor_name::XorName;

/// File APIs.
#[derive(Clone)]
pub struct Files {
    client: Client,
    wallet_dir: PathBuf,
}

type ChunkFileResult = Result<(XorName, u64, Vec<(XorName, PathBuf)>)>;

impl Files {
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

    #[instrument(skip(self), level = "debug")]
    /// Reads a file from the network, whose contents are contained within one or more chunks.
    pub async fn read_bytes(
        &self,
        address: ChunkAddress,
        downloaded_file_path: Option<PathBuf>,
    ) -> Result<Option<Bytes>> {
        let chunk = self.client.get_chunk(address).await?;

        // first try to deserialize a LargeFile, if it works, we go and seek it
        if let Ok(data_map) = self.unpack_chunk(chunk.clone()).await {
            self.read_all(data_map, downloaded_file_path).await
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
        // content_payments_map: ContentPaymentsMap,
        verify_store: bool,
    ) -> Result<NetworkAddress> {
        self.upload_bytes(bytes, verify_store).await
    }

    /// Tries to chunk the file, returning `(head_address, file_size, chunk_names)`
    /// and writes encrypted chunks to disk.
    #[instrument(skip_all, level = "trace")]
    pub fn chunk_file(&self, file_path: &Path, chunk_dir: &Path) -> ChunkFileResult {
        let mut file = File::open(file_path)?;
        let metadata = file.metadata()?;
        let file_size = metadata.len();

        let (head_address, chunks_paths) = if file_size < MIN_ENCRYPTABLE_BYTES as u64 {
            let mut bytes = Vec::new();
            let _ = file.read_to_end(&mut bytes)?;
            let chunk = package_small(SmallFile::new(bytes.into())?)?;

            // Write the result to disk
            let small_chunk_file_path = chunk_dir.join(hex::encode(*chunk.name()));
            let mut output_file = File::create(small_chunk_file_path.clone())?;
            output_file.write_all(&chunk.value)?;

            (*chunk.name(), vec![(*chunk.name(), small_chunk_file_path)])
        } else {
            encrypt_large(file_path, chunk_dir)?
        };
        Ok((head_address, file_size, chunks_paths))
    }

    /// Directly writes Chunks to the network in the
    /// form of immutable self encrypted chunks.
    ///
    #[instrument(skip_all, level = "trace")]
    pub async fn get_local_payment_and_upload_chunk(
        &self,
        chunk: Chunk,
        verify_store: bool,
        optional_permit: Option<OwnedSemaphorePermit>,
    ) -> Result<()> {
        let chunk_addr = chunk.network_address();
        trace!("Client upload started for chunk: {chunk_addr:?}");

        let wallet_client = self.wallet()?;
        let payment = wallet_client.get_payment_transfers(&chunk_addr)?;

        if payment.is_empty() {
            warn!("Failed to get payment proof for chunk: {chunk_addr:?} it was not found in the local wallet");
            return Err(Error::NoPaymentForRecord(PrettyPrintRecordKey::from(
                chunk_addr.to_record_key(),
            )))?;
        }

        trace!(
            "Payment for {chunk_addr:?}: has length: {:?}",
            payment.len()
        );
        self.client
            .store_chunk(chunk, payment, verify_store, optional_permit)
            .await?;

        trace!("Client upload completed for chunk: {chunk_addr:?}");
        Ok(())
    }

    /// Pay for a given set of chunks
    pub async fn pay_for_chunks(&self, chunks: Vec<XorName>, verify_store: bool) -> Result<()> {
        let mut wallet_client = self.wallet()?;
        info!("Paying for and uploading {:?} chunks", chunks.len());

        let cost = wallet_client
            .pay_for_storage(
                chunks.iter().map(|name| {
                    sn_protocol::NetworkAddress::ChunkAddress(ChunkAddress::new(*name))
                }),
                verify_store,
            )
            .await?;
        println!(
            "Successfully made payment of {cost} for {} chunks.)",
            chunks.len(),
        );

        if let Err(err) = wallet_client.store_local_wallet() {
            println!("Failed to store wallet: {err:?}");
        } else {
            println!(
                "Successfully stored wallet with cached payment proofs, and new balance {}.",
                wallet_client.balance()
            );
        }

        Ok(())
    }

    // --------------------------------------------
    // ---------- Private helpers -----------------
    // --------------------------------------------

    /// Used for testing
    #[instrument(skip(self, bytes), level = "trace")]
    async fn upload_bytes(&self, bytes: Bytes, verify: bool) -> Result<NetworkAddress> {
        let temp_dir = tempdir()?;
        let file_path = temp_dir.path().join("tempfile");
        let mut file = File::create(&file_path)?;
        file.write_all(&bytes)?;

        let chunk_path = temp_dir.path().join("chunk_path");
        create_dir_all(chunk_path.clone())?;

        let (head_address, _file_size, chunks_paths) = self.chunk_file(&file_path, &chunk_path)?;

        for (_chunk_name, chunk_path) in chunks_paths {
            let chunk = Chunk::new(Bytes::from(fs::read(chunk_path)?));
            self.get_local_payment_and_upload_chunk(chunk, verify, None)
                .await?;
        }

        Ok(NetworkAddress::ChunkAddress(ChunkAddress::new(
            head_address,
        )))
    }

    // Gets and decrypts chunks from the network using nothing else but the data map.
    // If a downloaded path is given, the decrypted file will be written to the given path,
    // by the decryptor directly.
    // Otherwise, will assume the fetched content is a small one and return as bytes.
    async fn read_all(
        &self,
        data_map: DataMap,
        decrypted_file_path: Option<PathBuf>,
    ) -> Result<Option<Bytes>> {
        let mut decryptor = if let Some(path) = decrypted_file_path {
            StreamSelfDecryptor::decrypt_to_file(Box::new(path), &data_map)?
        } else {
            let encrypted_chunks = self.try_get_chunks(data_map.infos()).await?;
            let bytes =
                decrypt_full_set(&data_map, &encrypted_chunks).map_err(Error::SelfEncryption)?;
            return Ok(Some(bytes));
        };

        let expected_count = data_map.infos().len();
        let mut missing_chunks = Vec::new();

        for (index, chunk_info) in data_map.infos().iter().enumerate() {
            match self
                .client
                .get_chunk(ChunkAddress::new(chunk_info.dst_hash))
                .await
            {
                Ok(chunk) => {
                    let encrypted_chunk = EncryptedChunk {
                        index: chunk_info.index,
                        content: chunk.value().clone(),
                    };
                    let _ = decryptor.next_encrypted(encrypted_chunk)?;
                }
                Err(err) => {
                    warn!(
                        "Reading chunk {} from network, resulted in error {err:?}.",
                        chunk_info.dst_hash
                    );
                    missing_chunks.push(chunk_info.dst_hash);
                }
            }
            info!("Client download progress {index:?}/{expected_count:?}");
            println!("Client download progress {index:?}/{expected_count:?}");
        }

        if !missing_chunks.is_empty() {
            Err(Error::NotEnoughChunksRetrieved {
                expected: expected_count,
                retrieved: expected_count - missing_chunks.len(),
                missing_chunks,
            })?
        } else {
            Ok(None)
        }
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
                    let serialized_chunk = self.read_all(data_map, None).await?.unwrap();
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

        let mut tasks = Vec::new();
        for chunk_info in chunks_info.clone().into_iter() {
            let client = self.client.clone();
            let task = task::spawn(async move {
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

/// Encrypts a [`LargeFile`] and returns the resulting address and all chunk names.
/// Correspondent encrypted chunks are writen in the specified output folder.
/// Does not store anything to the network.
fn encrypt_large(
    file_path: &Path,
    output_dir: &Path,
) -> Result<(XorName, Vec<(XorName, PathBuf)>)> {
    Ok(super::chunks::encrypt_large(file_path, output_dir)?)
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
