use bytes::Bytes;
use self_encryption::MIN_ENCRYPTABLE_BYTES;
use sn_protocol::storage::{Chunk, ChunkAddress};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use xor_name::XorName;

/// Tries to chunk the file, returning
/// `(head_address, data_map_chunk, file_size, chunk_names)`
/// and writes encrypted chunks to disk.
#[derive(Debug, PartialEq)]
pub struct ChunkOperator {
    pub head_chunk_address: ChunkAddress,
    pub data_map: Chunk,
    pub file_size: u64,
    pub chunk_vec: Vec<(XorName, PathBuf)>,
}

#[derive(Default, Clone)]
pub struct BuildChunkOperator {
    file_path: PathBuf,
    chunk_dir: PathBuf,
    file_size: u64,
    data_map_chunk: Chunk,
    chunk_vec: Vec<(XorName, PathBuf)>,
    // data_map_in_chunks: Vec<(XorName, PathBuf)>,
}

impl BuildChunkOperator {
    pub fn new() -> BuildChunkOperator {
        BuildChunkOperator {
            file_path: Default::default(),
            chunk_dir: Default::default(),
            file_size: 0,
            data_map_chunk: Default::default(),
            chunk_vec: vec![],
        }
    }

    fn append_data_map(&mut self) -> Vec<(XorName, PathBuf)> {
        let mut chunks_paths = self.chunk_vec.clone();
        let data_map_path = self
            .chunk_dir
            .join(hex::encode(*self.data_map_chunk.name()));
        let mut output_file =
            File::create(data_map_path.clone()).expect("error creating output file");

        // debug!("include_data_map_in_chunks {include_data_map_in_chunks:?}");
        info!("Data_map_chunk to be written!");
        trace!("Data_map_chunk being written to {data_map_path:?}");

        output_file
            .write_all(&self.data_map_chunk.value)
            .expect("error writing all");
        chunks_paths.push((*self.data_map_chunk.name(), data_map_path));
        chunks_paths
    }

    pub fn build(self, file_path: &Path, chunk_dir: &Path) -> ChunkOperator {
        let build = assign_values(self, file_path, chunk_dir);
        ChunkOperator {
            head_chunk_address: ChunkAddress::new(*build.data_map_chunk.name()),
            data_map: build.data_map_chunk.clone(),
            file_size: build.file_size,
            chunk_vec: build.chunk_vec,
        }
    }

    pub fn build_with_data_map_in_chunks(
        self,
        file_path: &Path,
        chunk_dir: &Path,
    ) -> ChunkOperator {
        let mut build = assign_values(self, file_path, chunk_dir);
        ChunkOperator {
            head_chunk_address: ChunkAddress::new(*build.data_map_chunk.name()),
            data_map: build.data_map_chunk.clone(),
            file_size: build.file_size,
            chunk_vec: build.append_data_map(),
        }
    }
}

fn small_file_size(file_size: u64) -> bool {
    file_size < MIN_ENCRYPTABLE_BYTES as u64
}

fn zero_file_size(file_size: u64) -> bool {
    file_size == 0
}

struct EmptyFile {}

struct SmallFile {
    buffer: Vec<u8>,
}

impl SmallFile {
    fn new(buffer: Vec<u8>) -> Self {
        Self { buffer }
    }
}

trait ChunkBySize {
    fn chunk_by_size(self) -> (Chunk, Vec<(XorName, PathBuf)>);
}

impl ChunkBySize for EmptyFile {
    fn chunk_by_size(self) -> (Chunk, Vec<(XorName, PathBuf)>) {
        let bytes: Bytes = Default::default();
        let chunk = Chunk::new(bytes);
        (chunk, vec![])
    }
}

impl ChunkBySize for SmallFile {
    fn chunk_by_size(self) -> (Chunk, Vec<(XorName, PathBuf)>) {
        let chunk = Chunk::new(Bytes::from(self.buffer));
        (chunk, vec![])
    }
}

fn encrypt_large(
    file_path: &Path,
    output_dir: &Path,
) -> crate::error::Result<(Chunk, Vec<(XorName, PathBuf)>)> {
    Ok(crate::chunks::encrypt_large(file_path, output_dir)?)
}

pub fn assign_values(
    mut build: BuildChunkOperator,
    file_path: &Path,
    chunk_dir: &Path,
) -> BuildChunkOperator {
    build.file_path = PathBuf::from(file_path);
    build.chunk_dir = PathBuf::from(chunk_dir);
    let mut file = File::open(build.file_path.clone()).expect("Unable to open file");
    build.file_size = file.metadata().expect("Unable to read file metadata").len();

    // Chunk file by filesize (refactor code to its own fn later)
    let (data_map_chunk, chunk_vec) = match build.file_size {
        file_size if zero_file_size(file_size) => EmptyFile {}.chunk_by_size(),
        file_size if small_file_size(file_size) => {
            let mut buffer = vec![0; file_size as usize];
            file.read_exact(&mut buffer)
                .expect("assign_values: read_exact failure");
            SmallFile::new(buffer).chunk_by_size()
        }
        _ => encrypt_large(
            &build.file_path.to_path_buf(),
            &build.chunk_dir.to_path_buf(),
        )
        .expect("assign_values: Can't encrypt"),
    };

    build.data_map_chunk = data_map_chunk;
    build.chunk_vec = chunk_vec;
    build
}
