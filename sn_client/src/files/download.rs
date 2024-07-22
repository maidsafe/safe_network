// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    chunks::{DataMapLevel, Error as ChunksError},
    error::{Error as ClientError, Result},
    Client, FilesApi, BATCH_SIZE,
};
use bytes::Bytes;
use futures::StreamExt;
use itertools::Itertools;
use self_encryption::{decrypt_full_set, DataMap, EncryptedChunk, StreamSelfDecryptor};
use sn_networking::target_arch::Instant;
use sn_protocol::storage::{Chunk, ChunkAddress, RetryStrategy};

use std::{collections::HashMap, fs, path::PathBuf};
use tokio::sync::mpsc::{self};
use xor_name::XorName;

/// The events emitted from the download process.
pub enum FilesDownloadEvent {
    /// Downloaded a Chunk from the network
    Downloaded(ChunkAddress),
    /// The total number of chunks we are about to download.
    /// Note: This count currently is not accurate. It does not take into account how we fetch the initial head chunk.
    ChunksCount(usize),
    /// The total number of data map chunks that we are about to download. This happens if the datamap file is.
    /// very large.
    /// Note: This count currently is not accurate. It does not take into account how we fetch the initial head chunk.
    DatamapCount(usize),
    /// The download process has terminated with an error.
    Error,
}

// Internally used to differentiate between the various ways that the downloaded chunks are returned.
enum DownloadReturnType {
    EncryptedChunks(Vec<EncryptedChunk>),
    DecryptedBytes(Bytes),
    WrittenToFileSystem,
}

/// `FilesDownload` provides functionality for downloading chunks with support for retries and queuing.
/// This struct is not cloneable. To create a new instance with default configuration, use the `new` function.
/// To modify the configuration, use the provided setter methods (`set_...` functions).
pub struct FilesDownload {
    // Configurations
    batch_size: usize,
    show_holders: bool,
    retry_strategy: RetryStrategy,
    // API
    api: FilesApi,
    // Events
    event_sender: Option<mpsc::Sender<FilesDownloadEvent>>,
    logged_event_sender_absence: bool,
}

impl FilesDownload {
    /// Creates a new instance of `FilesDownload` with the default configuration.
    /// To modify the configuration, use the provided setter methods (`set_...` functions).
    pub fn new(files_api: FilesApi) -> Self {
        Self {
            batch_size: BATCH_SIZE,
            show_holders: false,
            retry_strategy: RetryStrategy::Quick,
            api: files_api,
            event_sender: None,
            logged_event_sender_absence: false,
        }
    }

    /// Sets the default batch size that determines the number of chunks that are downloaded in parallel
    ///
    /// By default, this option is set to the constant `BATCH_SIZE: usize = 64`.
    pub fn set_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size;
        self
    }

    /// Sets the option to display the holders that are expected to be holding a chunk during verification.
    ///
    /// By default, this option is set to false.
    pub fn set_show_holders(mut self, show_holders: bool) -> Self {
        self.show_holders = show_holders;
        self
    }

    /// Sets the RetryStrategy to increase the re-try on failure attempts.
    ///
    /// By default, this option is set to RetryStrategy::Quick
    pub fn set_retry_strategy(mut self, retry_strategy: RetryStrategy) -> Self {
        self.retry_strategy = retry_strategy;
        self
    }

    /// Returns a receiver for file download events.
    /// This method is optional and the download process can be performed without it.
    pub fn get_events(&mut self) -> mpsc::Receiver<FilesDownloadEvent> {
        let (event_sender, event_receiver) = mpsc::channel(10);
        // should we return error if an sender is already set?
        self.event_sender = Some(event_sender);

        event_receiver
    }

    /// Download bytes from the network. The contents are spread across
    /// multiple chunks in the network. This function invokes the self-encryptor and returns
    /// the data that was initially stored.
    ///
    /// Takes `position` and `length` arguments which specify the start position
    /// and the length of bytes to be read.
    /// Passing `0` to position reads the data from the beginning,
    /// and the `length` is just an upper limit.
    pub async fn download_from(
        &mut self,
        address: ChunkAddress,
        position: usize,
        length: usize,
    ) -> Result<Bytes> {
        // clean up the trackers/stats
        self.logged_event_sender_absence = false;

        let result = self.download_from_inner(address, position, length).await;

        // send an event indicating that the download process completed with an error
        if result.is_err() {
            self.send_event(FilesDownloadEvent::Error).await?;
        }

        // drop the sender to close the channel.
        let sender = self.event_sender.take();
        drop(sender);

        result
    }

    pub async fn download_from_inner(
        &mut self,
        address: ChunkAddress,
        position: usize,
        length: usize,
    ) -> Result<Bytes> {
        debug!("Reading {length} bytes at: {address:?}, starting from position: {position}");
        let chunk = self
            .api
            .client
            .get_chunk(address, false, Some(self.retry_strategy))
            .await?;

        // First try to deserialize a LargeFile, if it works, we go and seek it.
        // If an error occurs, we consider it to be a SmallFile.
        if let Ok(data_map) = self.unpack_chunk(chunk.clone()).await {
            let info = self_encryption::seek_info(data_map.file_size(), position, length);
            let range = &info.index_range;
            let all_infos = data_map.infos();

            let to_download = (range.start..range.end + 1)
                .clone()
                .map(|i| all_infos[i].clone())
                .collect_vec();
            let to_download = DataMap::new(to_download);

            // not written to file and return the encrypted chunks
            if let DownloadReturnType::EncryptedChunks(encrypted_chunks) =
                self.read(to_download, None, true, false).await?
            {
                let bytes = self_encryption::decrypt_range(
                    &data_map,
                    &encrypted_chunks,
                    info.relative_pos,
                    length,
                )
                .map_err(ChunksError::SelfEncryption)?;
                return Ok(bytes);
            } else {
                error!("IncorrectDownloadOption: expected to get the encrypted chunks back");
                return Err(ClientError::IncorrectDownloadOption);
            }
        }

        // The error above is ignored to avoid leaking the storage format detail of SmallFiles and LargeFiles.
        // The basic idea is that we're trying to deserialize as one, and then the other.
        // The cost of it is that some errors will not be seen without a refactor.
        let mut bytes = chunk.value().clone();

        let _ = bytes.split_to(position);
        bytes.truncate(length);

        Ok(bytes)
    }

    /// Download a file from the network and get the decrypted bytes.
    /// If the data_map_chunk is not provided, the DataMap is fetched from the network using the provided address.
    pub async fn download_file(
        &mut self,
        address: ChunkAddress,
        data_map_chunk: Option<Chunk>,
    ) -> Result<Bytes> {
        if let Some(bytes) = self
            .download_entire_file(address, data_map_chunk, None)
            .await?
        {
            Ok(bytes)
        } else {
            error!("IncorrectDownloadOption: expected to get decrypted bytes, but we got None");
            Err(ClientError::IncorrectDownloadOption)
        }
    }

    /// Download a file from the network and write it to the provided path.
    /// If the data_map_chunk is not provided, the DataMap is fetched from the network using the provided address.
    pub async fn download_file_to_path(
        &mut self,
        address: ChunkAddress,
        data_map_chunk: Option<Chunk>,
        path: PathBuf,
    ) -> Result<()> {
        if self
            .download_entire_file(address, data_map_chunk, Some(path))
            .await?
            .is_none()
        {
            Ok(())
        } else {
            error!(
                "IncorrectDownloadOption: expected to not get any decrypted bytes, but got Some"
            );
            Err(ClientError::IncorrectDownloadOption)
        }
    }

    /// Download a file from the network.
    /// If you want to track the download progress, use the `get_events` method.
    async fn download_entire_file(
        &mut self,
        address: ChunkAddress,
        data_map_chunk: Option<Chunk>,
        downloaded_file_path: Option<PathBuf>,
    ) -> Result<Option<Bytes>> {
        // clean up the trackers/stats
        self.logged_event_sender_absence = false;

        let result = self
            .download_entire_file_inner(address, data_map_chunk, downloaded_file_path)
            .await;

        // send an event indicating that the download process completed with an error
        if result.is_err() {
            self.send_event(FilesDownloadEvent::Error).await?;
        }

        // drop the sender to close the channel.
        let sender = self.event_sender.take();
        drop(sender);

        result
    }

    async fn download_entire_file_inner(
        &mut self,
        address: ChunkAddress,
        data_map_chunk: Option<Chunk>,
        downloaded_file_path: Option<PathBuf>,
    ) -> Result<Option<Bytes>> {
        let head_chunk = if let Some(chunk) = data_map_chunk {
            info!("Downloading via supplied local datamap");
            chunk
        } else {
            match self
                .api
                .client
                .get_chunk(address, self.show_holders, Some(self.retry_strategy))
                .await
            {
                Ok(chunk) => chunk,
                Err(err) => {
                    error!("Failed to fetch head chunk {address:?}");
                    return Err(err);
                }
            }
        };

        // first try to deserialize a LargeFile, if it works, we go and seek it
        match self.unpack_chunk(head_chunk.clone()).await {
            Ok(data_map) => {
                // read_all emits
                match self
                    .read(data_map, downloaded_file_path, false, false)
                    .await?
                {
                    DownloadReturnType::EncryptedChunks(_) => {
                        error!("IncorrectDownloadOption: we should not be getting the encrypted chunks back as it is set to false.");
                        Err(ClientError::IncorrectDownloadOption)
                    }
                    DownloadReturnType::DecryptedBytes(bytes) => Ok(Some(bytes)),
                    DownloadReturnType::WrittenToFileSystem => Ok(None),
                }
            }
            Err(ClientError::Chunks(ChunksError::Deserialisation(_))) => {
                // Only in case of a deserialisation error,
                // shall consider the head chunk to be a SmallFile.
                // With the min-size now set to 3 Bytes, such case shall be rare.
                // Hence raise a warning for it.
                warn!("Consider head chunk {address:?} as an SmallFile");
                println!("Consider head chunk {address:?} as an SmallFile");

                self.send_event(FilesDownloadEvent::ChunksCount(1)).await?;
                self.send_event(FilesDownloadEvent::Downloaded(address))
                    .await?;
                if let Some(path) = downloaded_file_path {
                    fs::write(path, head_chunk.value().clone())?;
                    Ok(None)
                } else {
                    Ok(Some(head_chunk.value().clone()))
                }
            }
            Err(err) => {
                // For large data_map that consists of multiple chunks,
                // `unpack_chunk` function will try to fetch those chunks from network.
                // During the process, any chunk could be failed to download,
                // hence trigger an error to be raised.
                error!("Encounter error when unpack head_chunk {address:?} : {err:?}");
                println!("Encounter error when unpack head_chunk {address:?} : {err:?}");
                Err(err)
            }
        }
    }

    /// The internal logic to download the provided chunks inside the datamap.
    /// If the decrypted_file_path is provided, we return DownloadReturnType::WrittenToFileSystem
    /// If return_encrypted_chunks is true, we return DownloadReturnType::EncryptedChunks
    /// Else we return DownloadReturnType::DecryptedBytes
    ///
    /// Set we_are_downloading_a_datamap if we want to emit the DatamapCount else we emit ChunksCount
    async fn read(
        &mut self,
        data_map: DataMap,
        decrypted_file_path: Option<PathBuf>,
        return_encrypted_chunks: bool,
        we_are_downloading_a_datamap: bool,
    ) -> Result<DownloadReturnType> {
        // used internally
        enum DownloadKind {
            FileSystem(StreamSelfDecryptor),
            Memory(Vec<EncryptedChunk>),
        }

        let mut download_kind = {
            if let Some(path) = decrypted_file_path {
                DownloadKind::FileSystem(StreamSelfDecryptor::decrypt_to_file(
                    Box::new(path),
                    &data_map,
                )?)
            } else {
                DownloadKind::Memory(Vec::new())
            }
        };
        let chunk_infos = data_map.infos();
        let expected_count = chunk_infos.len();

        if we_are_downloading_a_datamap {
            self.send_event(FilesDownloadEvent::ChunksCount(expected_count))
                .await?;
        } else {
            // we're downloading the chunks related to a huge datamap
            self.send_event(FilesDownloadEvent::DatamapCount(expected_count))
                .await?;
        }

        let now = Instant::now();

        let client_clone = self.api.client.clone();
        let show_holders = self.show_holders;
        let retry_strategy = self.retry_strategy;
        // the initial index is not always 0 as we might seek a range of bytes. So fetch the first index
        let mut current_index = chunk_infos
            .first()
            .ok_or_else(|| ClientError::EmptyDataMap)?
            .index;
        let mut stream = futures::stream::iter(chunk_infos.into_iter())
            .map(|chunk_info| {
                Self::get_chunk(
                    client_clone.clone(),
                    chunk_info.dst_hash,
                    chunk_info.index,
                    show_holders,
                    retry_strategy,
                )
            })
            .buffer_unordered(self.batch_size);

        let mut chunk_download_cache = HashMap::new();

        while let Some(result) = stream.next().await {
            let (chunk_address, index, encrypted_chunk) = result?;
            // notify about the download
            self.send_event(FilesDownloadEvent::Downloaded(chunk_address))
                .await?;
            info!("Downloaded chunk of index {index:?}. We are at current_index {current_index:?}");

            // check if current_index is present in the cache before comparing the fetched index.
            // try to keep removing from the cache until we run out of sequential chunks to insert.
            while let Some(encrypted_chunk) = chunk_download_cache.remove(&current_index) {
                debug!("Got current_index {current_index:?} from the download cache. Incrementing current index");
                match &mut download_kind {
                    DownloadKind::FileSystem(decryptor) => {
                        let _ = decryptor.next_encrypted(encrypted_chunk)?;
                    }
                    DownloadKind::Memory(collector) => collector.push(encrypted_chunk),
                }
                current_index += 1;
            }
            // now check if we can process the fetched index, else cache it.
            if index == current_index {
                debug!("The downloaded chunk's index {index:?} matches the current index {current_index}. Processing it");
                match &mut download_kind {
                    DownloadKind::FileSystem(decryptor) => {
                        let _ = decryptor.next_encrypted(encrypted_chunk)?;
                    }
                    DownloadKind::Memory(collector) => collector.push(encrypted_chunk),
                }
                current_index += 1;
            } else {
                // since we download the chunks concurrently without order, we cache the results for an index that
                // finished earlier
                debug!("The downloaded chunk's index {index:?} does not match with the current_index {current_index}. Inserting into cache");
                let _ = chunk_download_cache.insert(index, encrypted_chunk);
            }
        }

        // finally empty out the cache.
        debug!("Finally emptying out the download cache");
        while let Some(encrypted_chunk) = chunk_download_cache.remove(&current_index) {
            debug!("Got current_index {current_index:?} from the download cache. Incrementing current index");
            match &mut download_kind {
                DownloadKind::FileSystem(decryptor) => {
                    let _ = decryptor.next_encrypted(encrypted_chunk)?;
                }
                DownloadKind::Memory(collector) => collector.push(encrypted_chunk),
            }
            current_index += 1;
        }
        if !chunk_download_cache.is_empty() {
            error!(
                "The chunk download cache is not empty. Current index {current_index:?}. The indices inside the cache: {:?}",
                chunk_download_cache.keys()
            );
            return Err(ClientError::FailedToAssembleDownloadedChunks);
        }

        let elapsed = now.elapsed();
        info!("Client downloaded file in {elapsed:?}");

        match download_kind {
            DownloadKind::FileSystem(_) => Ok(DownloadReturnType::WrittenToFileSystem),
            DownloadKind::Memory(collector) => {
                let result = if return_encrypted_chunks {
                    DownloadReturnType::EncryptedChunks(collector)
                } else {
                    let bytes = decrypt_full_set(&data_map, &collector)
                        .map_err(ChunksError::SelfEncryption)?;
                    DownloadReturnType::DecryptedBytes(bytes)
                };

                Ok(result)
            }
        }
    }

    /// Extracts a file DataMapLevel from a chunk.
    /// If the DataMapLevel is not the first level mapping directly to the user's contents,
    /// the process repeats itself until it obtains the first level DataMapLevel.
    pub async fn unpack_chunk(&mut self, mut chunk: Chunk) -> Result<DataMap> {
        loop {
            match rmp_serde::from_slice(chunk.value()).map_err(ChunksError::Deserialisation)? {
                DataMapLevel::First(data_map) => {
                    return Ok(data_map);
                }
                DataMapLevel::Additional(data_map) => {
                    if let DownloadReturnType::DecryptedBytes(serialized_chunk) =
                        self.read(data_map, None, false, true).await?
                    {
                        chunk = rmp_serde::from_slice(&serialized_chunk)
                            .map_err(ChunksError::Deserialisation)?;
                    } else {
                        error!("IncorrectDownloadOption: we should be getting the decrypted bytes back.");
                        return Err(ClientError::IncorrectDownloadOption);
                    }
                }
            }
        }
    }

    async fn send_event(&mut self, event: FilesDownloadEvent) -> Result<()> {
        if let Some(sender) = self.event_sender.as_ref() {
            sender.send(event).await.map_err(|err| {
                error!("Could not send files download event due to {err:?}");
                ClientError::CouldNotSendFilesEvent
            })?;
        } else if !self.logged_event_sender_absence {
            info!("Files download event sender is not set. Use get_events() if you need to keep track of the progress");
            self.logged_event_sender_absence = true;
        }
        Ok(())
    }

    async fn get_chunk(
        client: Client,
        address: XorName,
        index: usize,
        show_holders: bool,
        retry_strategy: RetryStrategy,
    ) -> std::result::Result<(ChunkAddress, usize, EncryptedChunk), ChunksError> {
        let chunk = client
            .get_chunk(
                ChunkAddress::new(address),
                show_holders,
                Some(retry_strategy),
            )
            .await
            .map_err(|err| {
                error!("Chunk missing {address:?} with {err:?}",);
                ChunksError::ChunkMissing(address)
            })?;
        let encrypted_chunk = EncryptedChunk {
            index,
            content: chunk.value,
        };
        Ok((chunk.address, index, encrypted_chunk))
    }
}
