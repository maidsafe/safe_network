// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod chunk;

pub use self::chunk::Chunk;

use super::{
    error::{Error, Result},
    list_files_in, prefix_tree_path, ChunkAddress,
};

use bytes::Bytes;
use hex::FromHex;
use std::{
    fmt::{self, Display, Formatter},
    io::{self, ErrorKind},
    path::{Path, PathBuf},
};
use tokio::{
    fs::{create_dir_all, read, remove_file, File},
    io::AsyncWriteExt,
};
use tracing::info;
use xor_name::XorName;

const CHUNKS_STORE_DIR_NAME: &str = "chunks";

/// Operations on data chunks.
#[derive(Clone, Debug)]
pub(crate) struct ChunkStorage {
    file_store_path: PathBuf,
}

impl ChunkStorage {
    /// Creates a new `ChunkStorage` at the specified root location
    ///
    /// If the location specified already contains a `ChunkStorage`, it is simply used
    pub(crate) fn new(path: &Path) -> Self {
        Self {
            file_store_path: path.join(CHUNKS_STORE_DIR_NAME),
        }
    }

    #[allow(unused)]
    pub(crate) fn addrs(&self) -> Vec<ChunkAddress> {
        list_files_in(&self.file_store_path)
            .iter()
            .filter_map(|filepath| Self::chunk_filepath_to_address(filepath).ok())
            .collect()
    }

    fn chunk_filepath_to_address(path: &Path) -> Result<ChunkAddress> {
        let filename = path
            .file_name()
            .ok_or_else(|| Error::NoFilename(path.to_path_buf()))?
            .to_str()
            .ok_or_else(|| Error::InvalidFilename(path.to_path_buf()))?;

        let xorname = XorName(<[u8; 32]>::from_hex(filename)?);
        Ok(ChunkAddress::new(xorname))
    }

    fn chunk_addr_to_filepath(&self, addr: &ChunkAddress) -> Result<PathBuf> {
        let xorname = *addr.name();
        let path = prefix_tree_path(&self.file_store_path, xorname);
        let filename = hex::encode(xorname);
        Ok(path.join(filename))
    }

    /// This is to be used when a node is shrinking the address range it is responsible for.
    #[allow(unused)]
    pub(super) async fn remove(&self, address: &ChunkAddress) -> Result<()> {
        debug!("Removing chunk, {:?}", address);
        let filepath = self.chunk_addr_to_filepath(address)?;
        remove_file(filepath).await?;
        Ok(())
    }

    pub(crate) async fn get(&self, address: &ChunkAddress) -> Result<Chunk> {
        trace!("Getting chunk {:?}", address);

        let file_path = self.chunk_addr_to_filepath(address)?;
        match read(file_path).await {
            Ok(bytes) => {
                let chunk = Chunk::new(Bytes::from(bytes));
                if chunk.address() != address {
                    // This can happen if the content read is empty, or incomplete,
                    // possibly due to an issue with the OS synchronising to disk,
                    // resulting in a mismatch with recreated address of the Chunk.
                    Err(Error::ChunkNotFound(*address))
                } else {
                    Ok(chunk)
                }
            }
            Err(io_error @ io::Error { .. }) if io_error.kind() == ErrorKind::NotFound => {
                Err(Error::ChunkNotFound(*address))
            }
            Err(other) => Err(other.into()),
        }
    }

    /// Store a chunk in the local disk store unless it is already there
    pub(crate) async fn store(&self, chunk: &Chunk) -> Result<()> {
        let addr = chunk.address();
        let filepath = self.chunk_addr_to_filepath(addr)?;

        if filepath.exists() {
            info!(
                "{}: Chunk data already exists, not storing: {:?}",
                self, addr
            );
            // Nothing more to do here
            return Ok(());
        }

        // Store the data on disk
        trace!("Storing chunk {addr:?}");
        if let Some(dirs) = filepath.parent() {
            create_dir_all(dirs).await?;
        }

        let mut file = File::create(filepath).await?;

        file.write_all(chunk.value()).await?;
        // Sync OS data to disk to reduce the chances of
        // concurrent reading failing by reading an empty/incomplete file.
        file.sync_data().await?;

        trace!("Stored new chunk {addr:?}");

        Ok(())
    }
}

impl Display for ChunkStorage {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "ChunkStorage")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use eyre::{eyre, Result};
    use futures::future::join_all;
    use rand::{rngs::OsRng, Rng};
    use rayon::{current_num_threads, prelude::*};
    use tempfile::tempdir;

    fn init_file_store() -> ChunkStorage {
        let root = tempdir().expect("Failed to create temporary directory for chunk disk store");
        ChunkStorage::new(root.path())
    }

    #[tokio::test]
    async fn test_write_read_chunk() {
        let storage = init_file_store();
        // Test that a range of different chunks return the written chunk.
        for _ in 0..10 {
            let chunk = Chunk::new(random_bytes(100));

            storage.store(&chunk).await.expect("Failed to write chunk.");

            let read_chunk = storage
                .get(chunk.address())
                .await
                .expect("Failed to read chunk.");

            assert_eq!(chunk.value(), read_chunk.value());
        }
    }

    #[tokio::test]
    async fn test_write_read_async_multiple_chunks() {
        let store = init_file_store();
        let size = 100;
        let chunks: Vec<Chunk> = std::iter::repeat_with(|| Chunk::new(random_bytes(size)))
            .take(7)
            .collect();
        write_and_read_chunks(&chunks, store).await;
    }

    #[tokio::test]
    async fn test_write_read_async_multiple_identical_chunks() {
        let store = init_file_store();
        let chunks: Vec<Chunk> = std::iter::repeat(Chunk::new(Bytes::from("test_concurrent")))
            .take(7)
            .collect();
        write_and_read_chunks(&chunks, store).await;
    }

    #[tokio::test]
    async fn test_read_chunk_empty_file() -> Result<()> {
        let storage = init_file_store();

        let chunk = Chunk::new(random_bytes(100));
        let address = chunk.address();

        // Create chunk file but with empty content.
        let filepath = storage.chunk_addr_to_filepath(address)?;
        if let Some(dirs) = filepath.parent() {
            create_dir_all(dirs).await?;
        }
        let mut file = File::create(&filepath).await?;
        file.write_all(b"").await?;

        // Trying to read the chunk shall return ChunkNotFound error since
        // its content shouldn't match chunk address.
        match storage.get(address).await {
            Ok(chunk) => Err(eyre!(
                "Unexpected Chunk read (size: {}): {chunk:?}",
                chunk.value().len()
            )),
            Err(Error::ChunkNotFound(addr)) => {
                assert_eq!(addr, *address, "Wrong Chunk address returned in error");
                Ok(())
            }
            Err(other) => Err(eyre!("Unexpected Error type returned: {other:?}")),
        }
    }

    async fn write_and_read_chunks(chunks: &[Chunk], storage: ChunkStorage) {
        // Write all chunks.
        let mut tasks = Vec::new();
        for c in chunks.iter() {
            tasks.push(async { storage.store(c).await.map(|_| *c.address()) });
        }
        let results = join_all(tasks).await;

        // Read all chunks.
        let tasks = results.iter().flatten().map(|addr| storage.get(addr));
        let results = join_all(tasks).await;
        let read_chunks: Vec<&Chunk> = results.iter().flatten().collect();

        // Verify all written were read.
        assert!(chunks
            .par_iter()
            .all(|c| read_chunks.iter().any(|r| r.value() == c.value())))
    }

    /// Generates a random vector using provided `length`.
    fn random_bytes(length: usize) -> Bytes {
        let threads = current_num_threads();

        if threads > length {
            let mut rng = OsRng;
            return ::std::iter::repeat(())
                .map(|()| rng.gen::<u8>())
                .take(length)
                .collect();
        }

        let per_thread = length / threads;
        let remainder = length % threads;

        let mut bytes: Vec<u8> = (0..threads)
            .par_bridge()
            .map(|_| vec![0u8; per_thread])
            .map(|mut bytes| {
                let bytes = bytes.as_mut_slice();
                rand::thread_rng().fill(bytes);
                bytes.to_owned()
            })
            .flatten()
            .collect();

        bytes.extend(vec![0u8; remainder]);

        Bytes::from(bytes)
    }
}
