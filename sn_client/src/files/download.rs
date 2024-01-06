// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    chunks::{DataMapLevel, Error as ChunksError},
    error::{Error as ClientError, Result},
    FilesApi, BATCH_SIZE, MAX_UPLOAD_RETRIES,
};
use bytes::Bytes;
use futures::{stream::FuturesOrdered, StreamExt};
use self_encryption::{decrypt_full_set, DataMap, EncryptedChunk, StreamSelfDecryptor};
use sn_protocol::storage::{Chunk, ChunkAddress};
use std::{fs, path::PathBuf, time::Instant};
use tokio::sync::mpsc::{self};

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

// Internally used to differentiate between downloading to a path/returning the bytes directly.
// This allows us to have a single function for both the download kinds.
enum DownloadKind {
    FileSystem(StreamSelfDecryptor),
    Bytes(Vec<EncryptedChunk>),
}

/// `FilesDownload` provides functionality for downloading chunks with support for retries and queuing.
/// This struct is not cloneable. To create a new instance with default configuration, use the `new` function.
/// To modify the configuration, use the provided setter methods (`set_...` functions).
pub struct FilesDownload {
    // Configurations
    batch_size: usize,
    show_holders: bool,
    // todo: controlled by GetRecordCfg, need to expose things.
    max_retries: usize,
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
            max_retries: MAX_UPLOAD_RETRIES,
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

    /// Sets the maximum number of retries to perform if a chunk fails to download.
    ///
    /// By default, this option is set to the constant `MAX_UPLOAD_RETRIES: usize = 3`.
    pub fn set_max_retries(mut self, max_retries: usize) -> Self {
        self.max_retries = max_retries;
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

    pub async fn download_file(
        &mut self,
        address: ChunkAddress,
        data_map_chunk: Option<Chunk>,
    ) -> Result<Bytes> {
        if let Some(bytes) = self.download(address, data_map_chunk, None).await? {
            Ok(bytes)
        } else {
            Err(ClientError::IncorrectDownloadOption)
        }
    }

    pub async fn download_file_to_path(
        &mut self,
        address: ChunkAddress,
        data_map_chunk: Option<Chunk>,
        path: PathBuf,
    ) -> Result<()> {
        if self
            .download(address, data_map_chunk, Some(path))
            .await?
            .is_some()
        {
            Err(ClientError::IncorrectDownloadOption)
        } else {
            Ok(())
        }
    }

    /// Download a file from the network.
    /// If you want to track the download progress, use the `get_events` method.
    async fn download(
        &mut self,
        address: ChunkAddress,
        data_map_chunk: Option<Chunk>,
        downloaded_file_path: Option<PathBuf>,
    ) -> Result<Option<Bytes>> {
        // clean up the trackers/stats
        self.logged_event_sender_absence = false;

        let result = self
            .read_bytes(address, data_map_chunk, downloaded_file_path)
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

    async fn read_bytes(
        &mut self,
        address: ChunkAddress,
        data_map_chunk: Option<Chunk>,
        downloaded_file_path: Option<PathBuf>,
    ) -> Result<Option<Bytes>> {
        let head_chunk = if let Some(chunk) = data_map_chunk {
            info!("Downloading via supplied local datamap");
            chunk
        } else {
            match self.api.client.get_chunk(address, self.show_holders).await {
                Ok(chunk) => chunk,
                Err(err) => {
                    error!("Failed to fetch head chunk {address:?}");
                    return Err(err);
                }
            }
        };

        // first try to deserialize a LargeFile, if it works, we go and seek it
        if let Ok(data_map) = self.unpack_chunk(head_chunk.clone()).await {
            // read_all emits
            self.read_all(data_map, downloaded_file_path, false).await
        } else {
            self.send_event(FilesDownloadEvent::ChunksCount(1)).await?;
            self.send_event(FilesDownloadEvent::Downloaded(address))
                .await?;
            // if an error occurs, we assume it's a SmallFile
            if let Some(path) = downloaded_file_path {
                fs::write(path, head_chunk.value().clone())?;
                Ok(None)
            } else {
                Ok(Some(head_chunk.value().clone()))
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

    // Gets and decrypts chunks from the network using nothing else but the data map.
    // If a downloaded path is given, the decrypted file will be written to the given path,
    // by the decryptor directly.
    // Otherwise, will assume the fetched content is a small one and return as bytes.
    async fn read_all(
        &mut self,
        data_map: DataMap,
        decrypted_file_path: Option<PathBuf>,
        we_are_downloading_a_datamap: bool,
    ) -> Result<Option<Bytes>> {
        let mut download_kind = {
            if let Some(path) = decrypted_file_path {
                DownloadKind::FileSystem(StreamSelfDecryptor::decrypt_to_file(
                    Box::new(path),
                    &data_map,
                )?)
            } else {
                DownloadKind::Bytes(Vec::new())
            }
        };

        if we_are_downloading_a_datamap {
            self.send_event(FilesDownloadEvent::ChunksCount(data_map.infos().len()))
                .await?;
        } else {
            // we're downloading the chunks related to a huge datamap
            self.send_event(FilesDownloadEvent::DatamapCount(data_map.infos().len()))
                .await?;
        }

        let expected_count = data_map.infos().len();
        let mut ordered_read_futures = FuturesOrdered::new();
        let now = Instant::now();
        let mut index = 0;
        let batch_size = self.batch_size;
        let client_clone = self.api.client.clone();
        let show_holders = self.show_holders;

        for chunk_info in data_map.infos().iter() {
            let dst_hash = chunk_info.dst_hash;
            let client_clone = client_clone.clone();

            // The futures are executed concurrently,
            // but the result is returned in the order in which they were inserted.
            ordered_read_futures.push_back(async move {
                (
                    dst_hash,
                    client_clone
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
                    // notify about the download
                    self.send_event(FilesDownloadEvent::Downloaded(chunk.address))
                        .await?;
                    let encrypted_chunk = EncryptedChunk {
                        index,
                        content: chunk.value().clone(),
                    };
                    match &mut download_kind {
                        DownloadKind::FileSystem(decryptor) => {
                            let _ = decryptor.next_encrypted(encrypted_chunk)?;
                        }
                        DownloadKind::Bytes(collector) => collector.push(encrypted_chunk),
                    }

                    index += 1;
                    info!("Client (read all) download progress {index:?}/{expected_count:?}");
                }
            }
        }

        let elapsed = now.elapsed();
        info!("Client downloaded file in {elapsed:?}");

        match download_kind {
            DownloadKind::FileSystem(_) => Ok(None),
            DownloadKind::Bytes(collector) => {
                let bytes =
                    decrypt_full_set(&data_map, &collector).map_err(ChunksError::SelfEncryption)?;
                Ok(Some(bytes))
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
                    let serialized_chunk = self
                        .read_all(data_map, None, true)
                        .await?
                        .expect("error encountered on reading additional datamap");
                    chunk = rmp_serde::from_slice(&serialized_chunk)
                        .map_err(ChunksError::Deserialisation)?;
                }
            }
        }
    }
}
