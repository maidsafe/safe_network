// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::get_progress_bar;
use super::upload::UploadedFile;
use bytes::Bytes;
use color_eyre::{
    eyre::{bail, eyre},
    Result,
};
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use sn_client::{
    protocol::storage::{Chunk, ChunkAddress},
    FilesApi,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};
use tracing::{debug, error, info, trace};
use walkdir::{DirEntry, WalkDir};
use xor_name::XorName;

const CHUNK_ARTIFACTS_DIR: &str = "chunk_artifacts";
const METADATA_FILE: &str = "metadata";

// The unique hex encoded hash(path)
// This allows us to uniquely identify if a file has been chunked or not.
// An alternative to use instead of filename as it might not be unique
#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
struct PathXorName(String);

impl PathXorName {
    fn new(path: &Path) -> PathXorName {
        // we just need an unique value per path, thus we don't have to mind between the
        // [u8]/[u16] differences
        let path_as_lossy_str = path.as_os_str().to_string_lossy();
        let path_xor = XorName::from_content(path_as_lossy_str.as_bytes());
        PathXorName(hex::encode(path_xor))
    }
}

/// Info about a file that has been chunked
#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct ChunkedFile {
    pub file_path: PathBuf,
    pub file_name: OsString,
    pub head_chunk_address: ChunkAddress,
    pub chunks: BTreeSet<(XorName, PathBuf)>,
    pub data_map: Chunk,
}

/// Manages the chunking process by resuming pre-chunked files and chunking any
/// file that has not been chunked yet.
#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct ChunkManager {
    /// Whole client root dir
    root_dir: PathBuf,
    /// Dir for chunk artifacts
    artifacts_dir: PathBuf,
    files_to_chunk: Vec<(OsString, PathXorName, PathBuf)>,
    chunks: BTreeMap<PathXorName, ChunkedFile>,
    completed_files: Vec<(PathBuf, OsString, ChunkAddress)>,
    resumed_chunk_count: usize,
    resumed_files_count: usize,
}

impl ChunkManager {
    // Provide the root_dir. The function creates a sub-directory to store the SE chunks
    pub fn new(root_dir: &Path) -> Self {
        let artifacts_dir = root_dir.join(CHUNK_ARTIFACTS_DIR);
        Self {
            root_dir: root_dir.to_path_buf(),
            artifacts_dir,
            files_to_chunk: Default::default(),
            chunks: Default::default(),
            completed_files: Default::default(),
            resumed_files_count: 0,
            resumed_chunk_count: 0,
        }
    }

    /// Chunk all the files in the provided `files_path`
    /// These are stored to the CHUNK_ARTIFACTS_DIR
    /// if read_cache is true, will take cache from previous runs into account
    ///
    /// # Arguments
    /// * files_path - &[Path]
    /// * read_cache - Boolean. Set to true to resume the chunks from the artifacts dir.
    /// * include_data_maps - Boolean. If set to true, will append all the ChunkedFile.data_map chunks
    pub fn chunk_path(
        &mut self,
        files_path: &Path,
        read_cache: bool,
        include_data_maps: bool,
    ) -> Result<()> {
        self.chunk_with_iter(
            WalkDir::new(files_path).into_iter().flatten(),
            read_cache,
            include_data_maps,
        )
    }

    /// Return the filename and the file's Xor address if all their chunks has been marked as
    /// verified
    pub(crate) fn already_put_chunks(
        &mut self,
        entries_iter: impl Iterator<Item = DirEntry>,
        make_files_public: bool,
    ) -> Result<Vec<(XorName, PathBuf)>> {
        self.chunk_with_iter(entries_iter, false, make_files_public)?;
        Ok(self.get_chunks())
    }

    /// Chunk all the files in the provided iterator
    /// These are stored to the CHUNK_ARTIFACTS_DIR
    /// if read_cache is true, will take cache from previous runs into account
    pub fn chunk_with_iter(
        &mut self,
        entries_iter: impl Iterator<Item = DirEntry>,
        read_cache: bool,
        include_data_maps: bool,
    ) -> Result<()> {
        let now = Instant::now();
        // clean up
        self.files_to_chunk = Default::default();
        self.chunks = Default::default();
        self.completed_files = Default::default();
        self.resumed_chunk_count = 0;
        self.resumed_files_count = 0;

        // collect the files to chunk
        entries_iter.for_each(|entry| {
            if entry.file_type().is_file() {
                let path_xor = PathXorName::new(entry.path());
                info!(
                    "Added file {:?} with path_xor: {path_xor:?} to be chunked/resumed",
                    entry.path()
                );
                self.files_to_chunk.push((
                    entry.file_name().to_owned(),
                    path_xor,
                    entry.into_path(),
                ));
            }
        });
        let total_files = self.files_to_chunk.len();

        if total_files == 0 {
            return Ok(());
        };

        // resume the chunks from the artifacts dir
        if read_cache {
            self.resume_path();
        }

        // note the number of chunks that we've resumed
        self.resumed_chunk_count = self
            .chunks
            .values()
            .flat_map(|chunked_file| &chunked_file.chunks)
            .count();
        // note the number of files that we've resumed
        self.resumed_files_count = self.chunks.keys().collect::<BTreeSet<_>>().len();

        // Filter out files_to_chunk; Any PathXorName in chunks_to_upload is considered to be resumed.
        {
            let path_xors = self.chunks.keys().collect::<BTreeSet<_>>();
            self.files_to_chunk
                .retain(|(_, path_xor, _)| !path_xors.contains(path_xor));
        }

        // Get the list of completed files
        {
            let completed_files = self.chunks.iter().filter_map(|(_, chunked_file)| {
                if chunked_file.chunks.is_empty() {
                    Some((
                        chunked_file.file_path.clone(),
                        chunked_file.file_name.clone(),
                        chunked_file.head_chunk_address,
                    ))
                } else {
                    None
                }
            });

            self.completed_files.extend(completed_files);
        }

        // Return early if no more files to chunk
        if self.files_to_chunk.is_empty() {
            debug!(
                "All files_to_chunk ({total_files:?}) were resumed. Returning the resumed chunks.",
            );
            debug!("It took {:?} to resume all the files", now.elapsed());
            return Ok(());
        }

        let progress_bar = get_progress_bar(total_files as u64)?;
        progress_bar.println(format!("Chunking {total_files} files..."));

        let artifacts_dir = &self.artifacts_dir.clone();
        let chunked_files = self.files_to_chunk
            .par_iter()
            .map(|(original_file_name, path_xor, path)| {
                let file_chunks_dir = {
                    let file_chunks_dir = artifacts_dir.join(&path_xor.0);
                    fs::create_dir_all(&file_chunks_dir).map_err(|err| {
                        error!("Failed to create folder {file_chunks_dir:?} for SE chunks with error {err:?}!");
                        eyre!("Failed to create dir {file_chunks_dir:?} for SE chunks with error {err:?}")
                    })?;
                    file_chunks_dir
                };

                match FilesApi::chunk_file(path, &file_chunks_dir, include_data_maps) {
                    Ok((head_chunk_address, data_map, size, chunks)) => {
                        progress_bar.clone().inc(1);
                        debug!("Chunked {original_file_name:?} with {path_xor:?} into file's XorName: {head_chunk_address:?} of size {size}, and chunks len: {}", chunks.len());

                        let chunked_file = ChunkedFile {
                            head_chunk_address,
                            file_path: path.to_owned(),
                            file_name: original_file_name.clone(),
                            chunks: chunks.into_iter().collect(),
                            data_map
                        };
                        Ok((path_xor.clone(), chunked_file))
                    }
                    Err(err) => {
                        println!("Failed to chunk file {path:?}/{path_xor:?} with err: {err:?}");
                        error!("Failed to chunk file {path:?}/{path_xor:?} with err: {err:?}");
                        Err(eyre!("Failed to chunk file {path:?}/{path_xor:?} with err: {err:?}"))
                    }
                }
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        debug!(
            "Out of total files_to_chunk {total_files}, we have resumed {} files and chunked {} files",
            self.resumed_files_count,
            chunked_files.len()
        );

        // Self::resume_path would create an empty self.chunks entry if a file that was fully
        // completed was resumed. Thus if it is empty, the user did not provide any valid file
        // path.
        if chunked_files.is_empty() && self.chunks.is_empty() {
            bail!(
                "The provided path does not contain any file. Please check your path!\nExiting..."
            );
        }

        // write metadata and data_map
        chunked_files
            .par_iter()
            .map(|(path_xor, chunked_file)| {
                let metadata_path = artifacts_dir.join(&path_xor.0).join(METADATA_FILE);

                info!("Metadata path is: {metadata_path:?}");
                let metadata = rmp_serde::to_vec(&(
                    chunked_file.head_chunk_address,
                    chunked_file.data_map.clone(),
                ))
                .map_err(|_| {
                    error!("Failed to serialize file_xor_addr for writing metadata");
                    eyre!("Failed to serialize file_xor_addr for writing metadata")
                })?;

                let mut metadata_file = File::create(&metadata_path).map_err(|_| {
                    error!("Failed to create metadata_path {metadata_path:?} for {path_xor:?}");
                    eyre!("Failed to create metadata_path {metadata_path:?} for {path_xor:?}")
                })?;

                metadata_file.write_all(&metadata).map_err(|_| {
                    error!("Failed to write metadata to {metadata_path:?} for {path_xor:?}");
                    eyre!("Failed to write metadata to {metadata_path:?} for {path_xor:?}")
                })?;

                debug!("Wrote metadata for {path_xor:?}");
                Ok(())
            })
            .collect::<Result<()>>()?;

        progress_bar.finish_and_clear();
        debug!("It took {:?} to chunk {} files", now.elapsed(), total_files);
        self.chunks.extend(chunked_files);

        Ok(())
    }

    // Try to resume the chunks
    fn resume_path(&mut self) {
        let artifacts_dir = self.artifacts_dir.clone();
        let resumed = self
            .files_to_chunk
            .par_iter()
            .filter_map(|(original_file_name, path_xor, original_file_path)| {
                // if this folder exists, and if we find chunks under this, we upload them.
                let file_chunks_dir = artifacts_dir.join(&path_xor.0);
                if !file_chunks_dir.exists() {
                    return None;
                }
                Self::read_file_chunks_dir(
                    file_chunks_dir,
                    path_xor,
                    original_file_path.clone(),
                    original_file_name.clone(),
                )
            })
            .collect::<BTreeMap<_, _>>();

        self.chunks.extend(resumed);
    }

    /// Get all the chunk name and their path.
    /// If include_data_maps is true, append all the ChunkedFile.data_map chunks to the vec
    pub fn get_chunks(&self) -> Vec<(XorName, PathBuf)> {
        self.chunks
            .values()
            .flat_map(|chunked_file| &chunked_file.chunks)
            .cloned()
            .collect::<Vec<(XorName, PathBuf)>>()
    }

    pub fn is_chunks_empty(&self) -> bool {
        self.chunks
            .values()
            .flat_map(|chunked_file| &chunked_file.chunks)
            .next()
            .is_none()
    }

    /// Mark all the chunks as completed. This removes the chunks from the CHUNK_ARTIFACTS_DIR.
    /// But keeps the folder and metadata file that denotes that the file has been already completed.
    #[allow(dead_code)]
    pub fn mark_completed_all(&mut self) -> Result<()> {
        let all_chunks = self
            .chunks
            .values()
            .flat_map(|chunked_file| &chunked_file.chunks)
            .map(|(chunk, _)| *chunk)
            .collect::<Vec<_>>();
        self.mark_completed(all_chunks.into_iter())
    }

    /// Mark a set of chunks as completed and remove them from CHUNK_ARTIFACTS_DIR
    /// If the entire file is completed, keep the folder and metadata file
    pub fn mark_completed(&mut self, chunks: impl Iterator<Item = XorName>) -> Result<()> {
        let set_of_completed_chunks = chunks.collect::<BTreeSet<_>>();
        trace!("marking as completed: {set_of_completed_chunks:?}");

        // remove those files
        self.chunks
            .par_iter()
            .flat_map(|(_, chunked_file)| &chunked_file.chunks)
            .map(|(chunk_xor, chunk_path)| {
                if set_of_completed_chunks.contains(chunk_xor) {
                    debug!("removing {chunk_xor:?} at {chunk_path:?} as it is marked as completed");
                    fs::remove_file(chunk_path).map_err(|_err| {
                        error!("Failed to remove SE chunk {chunk_xor} from {chunk_path:?}");
                        eyre!("Failed to remove SE chunk {chunk_xor} from {chunk_path:?}")
                    })?;
                }
                Ok(())
            })
            .collect::<Result<()>>()?;

        let mut entire_file_is_done = BTreeSet::new();
        // remove the entries from the struct
        self.chunks.iter_mut().for_each(|(path_xor, chunked_file)| {
            chunked_file
                .chunks
                // if chunk is part of completed_chunks, return false to remove it
                .retain(|(chunk_xor, _)| !set_of_completed_chunks.contains(chunk_xor));
            if chunked_file.chunks.is_empty() {
                entire_file_is_done.insert(path_xor.clone());
            }
        });

        for path_xor in &entire_file_is_done {
            // todo: should we remove the entry? ig so
            if let Some(chunked_file) = self.chunks.remove(path_xor) {
                trace!("removed {path_xor:?} from chunks list");

                self.completed_files.push((
                    chunked_file.file_path.clone(),
                    chunked_file.file_name.clone(),
                    chunked_file.head_chunk_address,
                ));

                let uploaded_file_metadata = UploadedFile {
                    filename: chunked_file.file_name,
                    data_map: Some(chunked_file.data_map.value),
                };
                // errors are logged by write()
                let _result =
                    uploaded_file_metadata.write(&self.root_dir, &chunked_file.head_chunk_address);
            }
        }
        Ok(())

        // let mut entire_file_is_done = BTreeSet::new();
        // // remove the entries from the struct
        // self.chunks.iter_mut().for_each(|(path_xor, chunked_file)| {
        //     chunked_file
        //         .chunks
        //         // if chunk is part of completed_chunks, return false to remove it
        //         .retain(|(chunk_xor, _)| !set_of_completed_chunks.contains(chunk_xor));
        //     if chunked_file.chunks.is_empty() {
        //         entire_file_is_done.insert(path_xor.clone());
        //     }
        // });

        // for path_xor in &entire_file_is_done {
        //     // todo: should we remove the entry? ig so
        //     if let Some(chunked_file) = self.chunks.remove(path_xor) {
        //         trace!("removed {path_xor:?} from chunks list");
        //         self.verified_files
        //             .push((chunked_file.file_name, chunked_file.head_chunk_address));
        //     }
        // }
    }

    /// Return the filename and the file's Xor address if all their chunks has been marked as
    /// completed
    pub(crate) fn completed_files(&self) -> &Vec<(PathBuf, OsString, ChunkAddress)> {
        &self.completed_files
    }

    /// Return the list of Filenames that have some chunks that are yet to be marked as completed.
    pub(crate) fn incomplete_files(&self) -> Vec<(&PathBuf, &OsString, &ChunkAddress)> {
        self.chunks
            .values()
            .map(|chunked_file| {
                (
                    &chunked_file.file_path,
                    &chunked_file.file_name,
                    &chunked_file.head_chunk_address,
                )
            })
            .collect()
    }

    /// Returns an iterator over the list of chunked files
    pub(crate) fn iter_chunked_files(&mut self) -> impl Iterator<Item = &ChunkedFile> {
        self.chunks.values()
    }

    // Try to read the chunks from `file_chunks_dir`
    // Returns the ChunkedFile if the metadata file exists
    // file_chunks_dir: artifacts_dir/path_xor
    // path_xor: Used during logging and is returned
    // original_file_name: Used to create ChunkedFile
    fn read_file_chunks_dir(
        file_chunks_dir: PathBuf,
        path_xor: &PathXorName,
        original_file_path: PathBuf,
        original_file_name: OsString,
    ) -> Option<(PathXorName, ChunkedFile)> {
        let mut file_chunk_address: Option<ChunkAddress> = None;
        let mut data_map = Chunk::new(Bytes::new());
        debug!("Trying to resume {path_xor:?} as the file_chunks_dir exists");

        let chunks = WalkDir::new(file_chunks_dir.clone())
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                if !entry.file_type().is_file() {
                    return None;
                }
                if entry.file_name() == METADATA_FILE {
                    if let Some((address, optional_data_map)) =
                        Self::try_read_metadata(entry.path())
                    {
                        file_chunk_address = Some(address);
                        data_map = optional_data_map;
                        debug!("Obtained metadata for {path_xor:?}");
                    } else {
                        error!("Could not read metadata for {path_xor:?}");
                    }
                    // not a chunk, so don't return
                    return None;
                }

                // try to get the chunk's xorname from its filename
                if let Some(file_name) = entry.file_name().to_str() {
                    Self::hex_decode_xorname(file_name)
                        .map(|chunk_xorname| (chunk_xorname, entry.into_path()))
                } else {
                    error!(
                        "Failed to convert OsString to str for {:?}",
                        entry.file_name()
                    );
                    None
                }
            })
            .collect::<BTreeSet<_>>();

        match file_chunk_address {
            Some(head_chunk_address) => {
                debug!("Resuming {} chunks for file {original_file_name:?} and with file_xor_addr {head_chunk_address:?}/{path_xor:?}", chunks.len());

                Some((
                    path_xor.clone(),
                    ChunkedFile {
                        file_path: original_file_path,
                        file_name: original_file_name,
                        head_chunk_address,
                        chunks,
                        data_map,
                    },
                ))
            }
            _ => {
                error!("Metadata file or data map was not present for {path_xor:?}");
                // metadata file or data map was not present/was not read
                None
            }
        }
    }

    /// Try to read the metadata file
    /// Returning (head_chunk_address, datamap Chunk)
    fn try_read_metadata(path: &Path) -> Option<(ChunkAddress, Chunk)> {
        let metadata = fs::read(path)
            .map_err(|err| error!("Failed to read metadata with err {err:?}"))
            .ok()?;
        // head chunk address and the final datamap contents if a datamap exists for this file
        let metadata: (ChunkAddress, Chunk) = rmp_serde::from_slice(&metadata)
            .map_err(|err| error!("Failed to deserialize metadata with err {err:?}"))
            .ok()?;

        Some(metadata)
    }

    // Decode the hex encoded xorname
    fn hex_decode_xorname(string: &str) -> Option<XorName> {
        let hex_decoded = hex::decode(string)
            .map_err(|err| error!("Failed to decode {string} into bytes with err {err:?}"))
            .ok()?;
        let decoded_xorname: [u8; xor_name::XOR_NAME_LEN] = hex_decoded
            .try_into()
            .map_err(|_| error!("Failed to convert hex_decoded xorname into an [u8; 32]"))
            .ok()?;
        Some(XorName(decoded_xorname))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use color_eyre::{eyre::eyre, Result};
    use rand::{thread_rng, Rng};
    use rayon::prelude::IntoParallelIterator;
    use sn_logging::LogBuilder;
    use tempfile::TempDir;

    /// Assert any collection/iterator even if their orders do not match.
    pub fn assert_list_eq<I, J, K>(a: I, b: J)
    where
        K: Eq + Clone,
        I: IntoIterator<Item = K>,
        J: IntoIterator<Item = K>,
    {
        let vec1: Vec<_> = a.into_iter().collect::<Vec<_>>();
        let mut vec2: Vec<_> = b.into_iter().collect();

        assert_eq!(vec1.len(), vec2.len());

        for item1 in &vec1 {
            let idx2 = vec2
                .iter()
                .position(|item2| item1 == item2)
                .expect("Item not found in second list");

            vec2.swap_remove(idx2);
        }

        assert_eq!(vec2.len(), 0);
    }

    #[test]
    fn chunked_files_should_be_written_to_artifacts_dir() -> Result<()> {
        let _log_guards = LogBuilder::init_single_threaded_tokio_test("chunk_manager");
        let (_tmp_dir, mut manager, _, random_files_dir) = init_manager()?;
        let artifacts_dir = manager.artifacts_dir.clone();
        let _ = create_random_files(&random_files_dir, 1, 1)?;
        manager.chunk_path(&random_files_dir, true, true)?;

        let chunks = manager.get_chunks();
        // 1. 1mb file produces 4 chunks
        assert_eq!(chunks.len(), 4);

        // 2. make sure we have 1 folder == 1 file
        let n_folders = WalkDir::new(&artifacts_dir)
            .into_iter()
            .flatten()
            .filter(|entry| entry.file_type().is_dir() && entry.path() != artifacts_dir)
            .count();
        assert_eq!(n_folders, 1);

        // 3. make sure we have the 1 files per chunk, + 1 datamap + 1 metadata file
        let n_files = WalkDir::new(&artifacts_dir)
            .into_iter()
            .flatten()
            .filter(|entry| {
                info!("direntry {entry:?}");
                entry.file_type().is_file()
            })
            .count();
        assert_eq!(n_files, chunks.len() + 1);

        // 4. make sure metadata file holds the correct file_xor_addr
        let mut file_xor_addr_from_metadata = None;
        for entry in WalkDir::new(&artifacts_dir).into_iter().flatten() {
            if entry.file_type().is_file() && entry.file_name() == METADATA_FILE {
                let metadata = ChunkManager::try_read_metadata(entry.path());

                if let Some((head_chunk_addr, _datamap)) = metadata {
                    file_xor_addr_from_metadata = Some(head_chunk_addr);
                }
            }
        }
        let file_xor_addr_from_metadata =
            file_xor_addr_from_metadata.expect("The metadata file should be present");
        let file_xor_addr = manager
            .chunks
            .values()
            .next()
            .expect("1 file should be present")
            .head_chunk_address;
        assert_eq!(file_xor_addr_from_metadata, file_xor_addr);

        // 5. make sure the chunked file's name is the XorName of that chunk
        let chunk_xornames = manager
            .chunks
            .values()
            .next()
            .expect("We must have 1 file here")
            .chunks
            .iter()
            .map(|(xor_name, _)| *xor_name)
            .collect::<BTreeSet<_>>();
        for entry in WalkDir::new(&artifacts_dir).into_iter().flatten() {
            let file_name = entry.file_name();
            if entry.file_type().is_file() && file_name != METADATA_FILE {
                let chunk_xorname_from_filename =
                    ChunkManager::hex_decode_xorname(file_name.to_str().unwrap())
                        .expect("Failed to get xorname from hex encoded file_name");
                assert!(chunk_xornames.contains(&chunk_xorname_from_filename));
            }
        }

        Ok(())
    }

    #[test]
    fn no_datamap_chunked_files_should_be_written_to_artifacts_dir_when_not_public() -> Result<()> {
        let _log_guards = LogBuilder::init_single_threaded_tokio_test("chunk_manager");
        let (_tmp_dir, mut manager, _, random_files_dir) = init_manager()?;
        let artifacts_dir = manager.artifacts_dir.clone();
        let _ = create_random_files(&random_files_dir, 1, 1)?;

        // we do NOT want to include or write the data_map chunk here
        manager.chunk_path(&random_files_dir, true, false)?;

        let chunks = manager.get_chunks();
        // 1. 1mb file produces 3 chunks without the datamap
        assert_eq!(chunks.len(), 3);

        // 2. make sure we have 1 folder == 1 file
        let n_folders = WalkDir::new(&artifacts_dir)
            .into_iter()
            .flatten()
            .filter(|entry| entry.file_type().is_dir() && entry.path() != artifacts_dir)
            .count();
        assert_eq!(n_folders, 1);

        // 3. make sure we have the 1 files per chunk, + 1 metadata file
        let n_files = WalkDir::new(&artifacts_dir)
            .into_iter()
            .flatten()
            .filter(|entry| {
                info!("direntry {entry:?}");
                entry.file_type().is_file()
            })
            .count();
        assert_eq!(n_files, chunks.len() + 1);

        // 4. make sure metadata file holds the correct file_xor_addr
        let mut file_xor_addr_from_metadata = None;
        for entry in WalkDir::new(&artifacts_dir).into_iter().flatten() {
            if entry.file_type().is_file() && entry.file_name() == METADATA_FILE {
                let metadata = ChunkManager::try_read_metadata(entry.path());

                if let Some((head_chunk_addr, _datamap)) = metadata {
                    file_xor_addr_from_metadata = Some(head_chunk_addr);
                }
            }
        }
        let file_xor_addr_from_metadata =
            file_xor_addr_from_metadata.expect("The metadata file should be present");
        let file_xor_addr = manager
            .chunks
            .values()
            .next()
            .expect("1 file should be present")
            .head_chunk_address;
        assert_eq!(file_xor_addr_from_metadata, file_xor_addr);

        // 5. make sure the chunked file's name is the XorName of that chunk
        let chunk_xornames = manager
            .chunks
            .values()
            .next()
            .expect("We must have 1 file here")
            .chunks
            .iter()
            .map(|(xor_name, _)| *xor_name)
            .collect::<BTreeSet<_>>();
        for entry in WalkDir::new(&artifacts_dir).into_iter().flatten() {
            let file_name = entry.file_name();
            if entry.file_type().is_file() && file_name != METADATA_FILE {
                let chunk_xorname_from_filename =
                    ChunkManager::hex_decode_xorname(file_name.to_str().unwrap())
                        .expect("Failed to get xorname from hex encoded file_name");
                assert!(chunk_xornames.contains(&chunk_xorname_from_filename));
            }
        }

        Ok(())
    }

    #[test]
    fn chunks_should_be_removed_from_artifacts_dir_if_marked_as_completed() -> Result<()> {
        let _log_guards = LogBuilder::init_single_threaded_tokio_test("chunk_manager");
        let (_tmp_dir, mut manager, _, random_files_dir) = init_manager()?;

        let _ = create_random_files(&random_files_dir, 1, 1)?;
        manager.chunk_path(&random_files_dir, true, true)?;

        let path_xor = manager.chunks.keys().next().unwrap().clone();
        let chunked_file = manager.chunks.values().next().unwrap().clone();
        let file_xor_addr = chunked_file.head_chunk_address;
        let (chunk, _) = chunked_file
            .chunks
            .first()
            .expect("Must contain 1 chunk")
            .clone();
        let total_chunks = manager.chunks.values().next().unwrap().chunks.len();
        manager.mark_completed(vec![chunk].into_iter())?;

        // 1. chunk should be removed from the struct
        assert_eq!(
            manager
                .chunks
                .values()
                .next()
                .expect("Since the file was not fully completed, it should be present")
                .chunks
                .len(),
            total_chunks - 1,
        );

        // 2. the folder should exists, but chunk removed
        let file_chunks_dir = manager.artifacts_dir.join(&path_xor.0);
        let (path_xor_from_dir, chunked_file_from_dir) = ChunkManager::read_file_chunks_dir(
            file_chunks_dir,
            &path_xor,
            chunked_file.file_path,
            chunked_file.file_name,
        )
        .expect("Folder and metadata should be present");
        assert_eq!(chunked_file_from_dir.chunks.len(), total_chunks - 1);
        assert_eq!(chunked_file_from_dir.head_chunk_address, file_xor_addr);
        assert_eq!(path_xor_from_dir, path_xor);

        // 2. file should not be marked as completed
        assert!(manager.completed_files.is_empty());

        Ok(())
    }

    #[test]
    fn marking_all_chunks_as_completed_should_not_remove_the_dir() -> Result<()> {
        let _log_guards = LogBuilder::init_single_threaded_tokio_test("chunk_manager");
        let (_tmp_dir, mut manager, _, random_files_dir) = init_manager()?;

        let _ = create_random_files(&random_files_dir, 5, 5)?;
        manager.chunk_path(&random_files_dir, true, true)?;
        // cloned after chunking
        let manager_clone = manager.clone();

        let n_folders = WalkDir::new(&manager.artifacts_dir)
            .into_iter()
            .flatten()
            .filter(|entry| entry.file_type().is_dir() && entry.path() != manager.artifacts_dir)
            .count();
        assert_eq!(n_folders, 5);

        manager.mark_completed_all()?;

        // all 5 files should be marked as completed
        assert_eq!(manager.completed_files.len(), 5);

        // all 5 folders should exist
        for (path_xor, chunked_file) in manager_clone.chunks.iter() {
            let file_chunks_dir = manager_clone.artifacts_dir.join(path_xor.0.clone());
            let (path_xor_from_dir, chunked_file_from_dir) = ChunkManager::read_file_chunks_dir(
                file_chunks_dir,
                path_xor,
                chunked_file.file_path.clone(),
                chunked_file.file_name.to_owned(),
            )
            .expect("Folder and metadata should be present");
            assert_eq!(chunked_file_from_dir.chunks.len(), 0);
            assert_eq!(
                chunked_file_from_dir.head_chunk_address,
                chunked_file.head_chunk_address
            );
            assert_eq!(&path_xor_from_dir, path_xor);
        }

        Ok(())
    }

    #[test]
    fn mark_none_and_resume() -> Result<()> {
        let _log_guards = LogBuilder::init_single_threaded_tokio_test("chunk_manager");
        let (_tmp_dir, mut manager, root_dir, random_files_dir) = init_manager()?;

        let _ = create_random_files(&random_files_dir, 5, 5)?;
        manager.chunk_path(&random_files_dir, true, true)?;

        let mut new_manager = ChunkManager::new(&root_dir);
        new_manager.chunk_path(&random_files_dir, true, true)?;

        // 1. make sure the chunk counts match
        let total_chunk_count = manager
            .chunks
            .values()
            .flat_map(|chunked_file| &chunked_file.chunks)
            .count();
        assert_eq!(manager.resumed_chunk_count, 0);
        assert_eq!(new_manager.resumed_chunk_count, total_chunk_count);

        // 2. assert the two managers
        assert_eq!(manager.chunks, new_manager.chunks);
        assert_eq!(manager.completed_files, new_manager.completed_files);

        Ok(())
    }

    #[test]
    fn mark_one_chunk_and_resume() -> Result<()> {
        let _log_guards = LogBuilder::init_single_threaded_tokio_test("chunk_manager");
        let (_tmp_dir, mut manager, root_dir, random_files_dir) = init_manager()?;

        let _ = create_random_files(&random_files_dir, 5, 5)?;
        manager.chunk_path(&random_files_dir, true, true)?;

        let total_chunks_count = manager
            .chunks
            .values()
            .flat_map(|chunked_file| &chunked_file.chunks)
            .count();

        // mark a chunk as completed
        let removed_chunk = manager
            .chunks
            .values()
            .next()
            .expect("Atleast 1 file should be present")
            .chunks
            .iter()
            .next()
            .expect("Chunk should be present")
            .0;
        manager.mark_completed([removed_chunk].into_iter())?;
        let mut new_manager = ChunkManager::new(&root_dir);
        new_manager.chunk_path(&random_files_dir, true, true)?;

        // 1. we should have 1 completed chunk and (total_chunks_count-1) incomplete chunks
        assert_eq!(manager.resumed_chunk_count, 0);
        assert_eq!(new_manager.resumed_chunk_count, total_chunks_count - 1);
        // also check the structs
        assert_eq!(
            new_manager
                .chunks
                .values()
                .flat_map(|chunked_file| &chunked_file.chunks)
                .count(),
            total_chunks_count - 1
        );

        // 2. files should not be added to completed files
        assert_eq!(new_manager.completed_files.len(), 0);

        Ok(())
    }

    #[test]
    fn mark_all_and_resume() -> Result<()> {
        let _log_guards = LogBuilder::init_single_threaded_tokio_test("chunk_manager");
        let (_tmp_dir, mut manager, root_dir, random_files_dir) = init_manager()?;

        let _ = create_random_files(&random_files_dir, 5, 5)?;
        manager.chunk_path(&random_files_dir, true, true)?;
        manager.mark_completed_all()?;

        let mut new_manager = ChunkManager::new(&root_dir);
        new_manager.chunk_path(&random_files_dir, true, true)?;

        // 1. we should have chunk entries, but 0 chunks inside them
        assert_eq!(new_manager.chunks.len(), 5);
        assert_eq!(
            new_manager
                .chunks
                .values()
                .flat_map(|chunked_file| &chunked_file.chunks)
                .count(),
            0
        );
        // 2. the resumed stats should be 0
        assert_eq!(new_manager.resumed_chunk_count, 0);

        // 3. make sure the files are added to completed list
        assert_eq!(new_manager.completed_files.len(), 5);

        Ok(())
    }

    #[test]
    fn absence_of_metadata_file_should_re_chunk_the_entire_file() -> Result<()> {
        let _log_guards = LogBuilder::init_single_threaded_tokio_test("chunk_manager");
        let (_tmp_dir, mut manager, _root_dir, random_files_dir) = init_manager()?;

        let mut random_files = create_random_files(&random_files_dir, 1, 1)?;
        let random_file = random_files.remove(0);
        manager.chunk_path(&random_files_dir, true, true)?;

        let mut old_chunks_list = BTreeSet::new();
        for entry in WalkDir::new(&manager.artifacts_dir).into_iter().flatten() {
            let file_name = entry.file_name();
            if entry.file_type().is_file() && file_name != METADATA_FILE {
                let chunk_xorname_from_filename =
                    ChunkManager::hex_decode_xorname(file_name.to_str().unwrap())
                        .expect("Failed to get xorname from hex encoded file_name");
                old_chunks_list.insert(chunk_xorname_from_filename);
            }
        }

        // remove metadata file from artifacts_dir
        let path_xor = PathXorName::new(&random_file);
        let metadata_path = manager.artifacts_dir.join(path_xor.0).join(METADATA_FILE);
        fs::remove_file(&metadata_path)?;

        // use the same manager to chunk the path
        manager.chunk_path(&random_files_dir, true, true)?;
        // nothing should be resumed
        assert_eq!(manager.resumed_chunk_count, 0);
        // but it should be re-chunked
        assert_eq!(
            manager.get_chunks().len(),
            4,
            "we have correct chunk len including data_map"
        );
        // metadata file should be created
        assert!(metadata_path.exists());

        let mut new_chunks_list = BTreeSet::new();
        for entry in WalkDir::new(&manager.artifacts_dir).into_iter().flatten() {
            let file_name = entry.file_name();
            if entry.file_type().is_file() && file_name != METADATA_FILE {
                let chunk_xorname_from_filename =
                    ChunkManager::hex_decode_xorname(file_name.to_str().unwrap())
                        .expect("Failed to get xorname from hex encoded file_name");
                new_chunks_list.insert(chunk_xorname_from_filename);
            }
        }
        assert_list_eq(new_chunks_list, old_chunks_list);

        Ok(())
    }

    fn init_manager() -> Result<(TempDir, ChunkManager, PathBuf, PathBuf)> {
        let tmp_dir = tempfile::tempdir()?;
        let random_files_dir = tmp_dir.path().join("random_files");
        let root_dir = tmp_dir.path().join("root_dir");
        fs::create_dir_all(&random_files_dir)?;
        fs::create_dir_all(&root_dir)?;
        let manager = ChunkManager::new(&root_dir);

        Ok((tmp_dir, manager, root_dir, random_files_dir))
    }

    fn create_random_files(
        at: &Path,
        num_files: usize,
        mb_per_file: usize,
    ) -> Result<Vec<PathBuf>> {
        let files = (0..num_files)
            .into_par_iter()
            .filter_map(|i| {
                let mut path = at.to_path_buf();
                path.push(format!("random_file_{i}"));
                match generate_file(&path, mb_per_file) {
                    Ok(_) => Some(path),
                    Err(err) => {
                        error!("Failed to generate random file with {err:?}");
                        None
                    }
                }
            })
            .collect::<Vec<_>>();
        if files.len() < num_files {
            return Err(eyre!("Failed to create a Failedkk"));
        }
        Ok(files)
    }

    fn generate_file(path: &PathBuf, file_size_mb: usize) -> Result<()> {
        let mut file = File::create(path)?;
        let mut rng = thread_rng();

        // can create [u8; 32] max at time. Thus each mb has 1024*32 such small chunks
        let n_small_chunks = file_size_mb * 1024 * 32;
        for _ in 0..n_small_chunks {
            let random_data: [u8; 32] = rng.gen();
            file.write_all(&random_data)?;
        }
        let size = file.metadata()?.len() as f64 / (1024 * 1024) as f64;
        assert_eq!(file_size_mb as f64, size);

        Ok(())
    }
}
