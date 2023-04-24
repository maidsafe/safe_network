// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::ChunkAddress;

use bytes::Bytes;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use xor_name::XorName;

/// Chunk, an immutable chunk of data
#[derive(Hash, Eq, PartialEq, PartialOrd, Ord, Clone, custom_debug::Debug)]
pub struct Chunk {
    /// Network address. Omitted when serialising and
    /// calculated from the `value` when deserialising.
    address: ChunkAddress,
    /// Contained data.
    #[debug(skip)]
    value: Bytes,
}

impl Chunk {
    /// Creates a new instance of `Chunk`.
    pub fn new(value: Bytes) -> Self {
        Self {
            address: ChunkAddress::new(XorName::from_content(value.as_ref())),
            value,
        }
    }

    /// Returns the value.
    pub fn value(&self) -> &Bytes {
        &self.value
    }

    /// Returns the address.
    pub fn address(&self) -> &ChunkAddress {
        &self.address
    }

    /// Returns the name.
    pub fn name(&self) -> &XorName {
        self.address.name()
    }

    /// Returns size of contained value.
    pub fn payload_size(&self) -> usize {
        self.value.len()
    }

    /// Returns size of this chunk after serialisation.
    pub fn serialised_size(&self) -> usize {
        self.value.len()
    }
}

impl Serialize for Chunk {
    fn serialize<S: Serializer>(&self, serialiser: S) -> Result<S::Ok, S::Error> {
        // Address is omitted since it's derived from value
        self.value.serialize(serialiser)
    }
}

impl<'de> Deserialize<'de> for Chunk {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Deserialize::deserialize(deserializer)?;
        Ok(Self::new(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use assert_fs::TempDir;
    use eyre::{eyre, Result};
    use futures::future::join_all;
    use rand::{rngs::OsRng, Rng};
    use rayon::{current_num_threads, prelude::*};

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

    fn init_file_store() -> ChunkStorage {
        let root = TempDir::new().expect("Should be able to create a temp dir.");
        ChunkStorage::new(root.path())
    }
}
