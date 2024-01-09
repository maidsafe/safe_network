// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

pub(crate) mod download;
pub(crate) mod upload;

use crate::{chunks::Error as ChunksError, error::Result, Client, WalletClient};
use bytes::Bytes;
use libp2p::PeerId;
use self_encryption::{self, MIN_ENCRYPTABLE_BYTES};
use sn_protocol::{
    storage::{Chunk, ChunkAddress},
    NetworkAddress,
};
use sn_transfers::{LocalWallet, NanoTokens};
use std::{
    fs::{self, create_dir_all, File},
    io::Write,
    path::{Path, PathBuf},
};
use tempfile::tempdir;
use tracing::trace;
use xor_name::XorName;

/// `BATCH_SIZE` determines the number of chunks that are processed in parallel during the payment and upload process.
pub const BATCH_SIZE: usize = 16;

/// The maximum number of retries to perform on a failed chunk.
pub const MAX_UPLOAD_RETRIES: usize = 3;

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

    /// Tries to chunk the file, returning `(head_address, data_map_chunk, file_size, chunk_names)`
    /// and writes encrypted chunks to disk.
    pub fn chunk_file(
        file_path: &Path,
        chunk_dir: &Path,
        include_data_map_in_chunks: bool,
    ) -> ChunkFileResult {
        let file = File::open(file_path)?;
        let metadata = file.metadata()?;
        let file_size = metadata.len();

        let (head_address, data_map_chunk, mut chunks_paths) =
            if file_size < MIN_ENCRYPTABLE_BYTES as u64 {
                Err(ChunksError::FileTooSmall)?
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
}

/// Encrypts a [`LargeFile`] and returns the resulting address and all chunk names.
/// Correspondent encrypted chunks are written in the specified output folder.
/// Does not store anything to the network.
///
/// Returns data map as a chunk, and the resulting chunks
fn encrypt_large(file_path: &Path, output_dir: &Path) -> Result<(Chunk, Vec<(XorName, PathBuf)>)> {
    Ok(crate::chunks::encrypt_large(file_path, output_dir)?)
}
