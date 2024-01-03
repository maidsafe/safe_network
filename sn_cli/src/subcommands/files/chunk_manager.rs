// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::subcommands::files::get_progress_bar;
use bytes::Bytes;
use color_eyre::{eyre::bail, Result};
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use sn_client::FilesApi;
use sn_protocol::storage::ChunkAddress;
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};
use walkdir::WalkDir;
use xor_name::XorName;

const CHUNK_ARTIFACTS_DIR: &str = "chunk_artifacts";
const METADATA_FILE: &str = "metadata";

/// Subdir for storing uploaded file indo
pub(crate) const UPLOADED_FILES: &str = "uploaded_files";

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
pub(crate) struct ChunkedFile {
    pub file_name: OsString,
    pub head_chunk_address: ChunkAddress,
    pub chunks: BTreeSet<(XorName, PathBuf)>,
    pub data_map: Option<Bytes>,
}

/// Manages the chunking process by resuming pre-chunked files and chunking any
/// file that has not been chunked yet.
#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub(crate) struct ChunkManager {
    /// Whole client root dir
    root_dir: PathBuf,
    /// Dir for chunk artefacts
    artifacts_dir: PathBuf,
    files_to_chunk: Vec<(OsString, PathXorName, PathBuf)>,
    chunks: BTreeMap<PathXorName, ChunkedFile>,
    verified_files: Vec<(OsString, ChunkAddress)>,
    resumed_chunk_count: usize,
    resumed_files_count: usize,
}

impl ChunkManager {
    // Provide the root_dir. The function creates a sub-directory to store the SE chunks
    pub(crate) fn new(root_dir: &Path) -> Self {
        let artifacts_dir = root_dir.join(CHUNK_ARTIFACTS_DIR);
        Self {
            root_dir: root_dir.to_path_buf(),
            artifacts_dir,
            files_to_chunk: Default::default(),
            chunks: Default::default(),
            verified_files: Default::default(),
            resumed_files_count: 0,
            resumed_chunk_count: 0,
        }
    }

    /// Chunk all the files in the provided `files_path`
    /// These are stored to the CHUNK_ARTIFACTS_DIR
    /// if read_cache is true, will take cache from previous runs into account
    pub(crate) fn chunk_path(
        &mut self,
        files_path: &Path,
        read_cache: bool,
        include_data_maps: bool,
    ) -> Result<()> {
        println!("Starting to chunk {files_path:?} now.");
        let now = Instant::now();
        // clean up
        self.files_to_chunk = Default::default();
        self.chunks = Default::default();
        self.verified_files = Default::default();
        self.resumed_chunk_count = 0;
        self.resumed_files_count = 0;

        // collect the files to chunk
        WalkDir::new(files_path)
            .into_iter()
            .flatten()
            .for_each(|entry| {
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

        // Get the list of verified files
        {
            let verified_files = self.chunks.iter().filter_map(|(_, chunked_file)| {
                if chunked_file.chunks.is_empty() {
                    Some((
                        chunked_file.file_name.clone(),
                        chunked_file.head_chunk_address,
                    ))
                } else {
                    None
                }
            });

            self.verified_files.extend(verified_files);
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
            .filter_map(|(original_file_name, path_xor, path)| {
                let file_chunks_dir = {
                    let file_chunks_dir = artifacts_dir.join(&path_xor.0);
                    match fs::create_dir_all(&file_chunks_dir) {
                        Ok(_) => file_chunks_dir,
                        Err(err) => {
                            println!("Failed to create temp folder {file_chunks_dir:?} for SE chunks with error {err:?}!");
                            error!("Failed to create temp folder {file_chunks_dir:?} for SE chunks with error {err:?}!");
                            // use the chunk_artifacts_dir directly; This should not result in any
                            // undefined behaviour. The resume operation will be disabled if we don't
                            // use the `path_xor` dir.
                            // TODO: maybe error out if we get any fs errors.
                            artifacts_dir.clone()
                        }
                    }
                };

                match FilesApi::chunk_file(path, &file_chunks_dir, include_data_maps) {
                    Ok((head_chunk_address, data_map, size, chunks)) => {
                        progress_bar.clone().inc(1);
                        debug!("Chunked {original_file_name:?} with {path_xor:?} into file's XorName: {head_chunk_address:?} of size {size}, and chunks len: {}", chunks.len());

                        let chunked_file = ChunkedFile {
                            head_chunk_address,
                            file_name: original_file_name.clone(),
                            chunks: chunks.into_iter().collect(),
                            data_map
                        };
                        Some((path_xor.clone(), chunked_file))
                    }
                    Err(err) => {
                        println!("Skipping file {path:?}/{path_xor:?} as it could not be chunked: {err:?}");
                        error!("Skipping file {path:?}/{path_xor:?} as it could not be chunked: {err:?}");
                        None
                    }
                }
            })
            .collect::<BTreeMap<_, _>>();
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
        let _ = chunked_files
            .par_iter()
            .filter_map(|(path_xor, chunked_file)| {
                let metadata_path = artifacts_dir.join(&path_xor.0).join(METADATA_FILE);

                info!("Metadata path is: {metadata_path:?}");
                let metadata = rmp_serde::to_vec(&(
                    chunked_file.head_chunk_address,
                    chunked_file.data_map.clone(),
                ))
                .map_err(|_| error!("Failed to serialize file_xor_addr for writing metadata"))
                .ok()?;

                let mut metadata_file = File::create(&metadata_path)
                    .map_err(|_| {
                        error!("Failed to create metadata_path {metadata_path:?} for {path_xor:?}")
                    })
                    .ok()?;

                metadata_file
                    .write_all(&metadata)
                    .map_err(|_| {
                        error!("Failed to write metadata to {metadata_path:?} for {path_xor:?}")
                    })
                    .ok()?;

                debug!("Wrote metadata for {path_xor:?}");
                Some(())
            })
            .count();

        progress_bar.finish_and_clear();
        debug!(
            "It took {:?} to chunk {} files",
            now.elapsed(),
            self.files_to_chunk.len()
        );
        self.chunks.extend(chunked_files);

        Ok(())
    }

    // Try to resume the chunks
    fn resume_path(&mut self) {
        let artifacts_dir = self.artifacts_dir.clone();
        let resumed = self
            .files_to_chunk
            .par_iter()
            .filter_map(|(original_file_name, path_xor, _)| {
                // if this folder exists, and if we find chunks under this, we upload them.
                let file_chunks_dir = artifacts_dir.join(&path_xor.0);
                if !file_chunks_dir.exists() {
                    return None;
                }
                Self::read_file_chunks_dir(file_chunks_dir, path_xor, original_file_name.clone())
            })
            .collect::<BTreeMap<_, _>>();

        self.chunks.extend(resumed);
    }

    /// Get all the chunk name and their path.
    /// If include_data_maps is true, append all the ChunkedFile.data_map chunks to the vec
    pub(crate) fn get_chunks(&self) -> Vec<(XorName, PathBuf)> {
        self.chunks
            .values()
            .flat_map(|chunked_file| &chunked_file.chunks)
            .cloned()
            .collect::<Vec<(XorName, PathBuf)>>()
    }

    pub(crate) fn is_chunks_empty(&self) -> bool {
        self.chunks
            .values()
            .flat_map(|chunked_file| &chunked_file.chunks)
            .next()
            .is_none()
    }

    /// Mark all the chunks as completed. This removes the chunks from the CHUNK_ARTIFACTS_DIR.
    /// But keeps the folder and metadata file that denotes that the file has been already completed.
    #[allow(dead_code)]
    pub(crate) fn mark_completed_all(&mut self) {
        let all_chunks = self
            .chunks
            .values()
            .flat_map(|chunked_file| &chunked_file.chunks)
            .map(|(chunk, _)| *chunk)
            .collect::<Vec<_>>();
        self.mark_completed(all_chunks.into_iter());
    }

    /// Mark a set of chunks as completed and remove them from CHUNK_ARTIFACTS_DIR
    /// If the entire file is completed, keep the folder and metadata file
    pub(crate) fn mark_completed(&mut self, chunks: impl Iterator<Item = XorName>) {
        let set_of_completed_chunks = chunks.collect::<BTreeSet<_>>();
        trace!("marking as completed: {set_of_completed_chunks:?}");

        // remove those files
        let _ = self
            .chunks
            .par_iter()
            .flat_map(|(_, chunked_file)| &chunked_file.chunks)
            .filter_map(|(chunk_xor, chunk_path)| {
                if set_of_completed_chunks.contains(chunk_xor) {
                    debug!("removing {chunk_xor:?} at {chunk_path:?} as it is marked as completed");
                    fs::remove_file(chunk_path)
                        .map_err(|_err| {
                            error!("Failed to remove SE chunk {chunk_xor} from {chunk_path:?}");
                        })
                        .ok()?;
                }
                Some(())
            })
            .count();

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
                self.verified_files.push((
                    chunked_file.file_name.clone(),
                    chunked_file.head_chunk_address,
                ));

                // write the data_map addr and or data_map to the UPLOADED_FILES dir
                let filename_hex = chunked_file.head_chunk_address.to_hex();

                // safely create the filename
                let safe_filename = match chunked_file.file_name.to_str() {
                    Some(name) => format!("{name}::{filename_hex}"),
                    None => format!("::{filename_hex}"),
                };

                // ensure self.root_dir.join(UPLOADED_FILES) exists
                let uploaded_files = self.root_dir.join(UPLOADED_FILES);
                if !uploaded_files.exists() {
                    if let Err(error) = fs::create_dir_all(&uploaded_files) {
                        error!("Failed to create {uploaded_files:?} because {error:?}");
                    }
                }

                let uploaded_file = uploaded_files.join(&safe_filename);

                warn!(
                    "Marking {uploaded_file:?} as completed for chunked_file {:?}",
                    chunked_file
                );

                if let Some(data_map) = &chunked_file.data_map {
                    info!(
                        "Datamap to write for {:?} is {:?} bytes",
                        chunked_file.file_name,
                        data_map.len()
                    );

                    if let Err(error) = fs::write(uploaded_file, data_map) {
                        error!(
                            "Could not write datamap for {:?}, {error:?}",
                            chunked_file.head_chunk_address
                        );
                    }
                } else {
                    warn!(
                        "No datamap being written for {:?} as it is empty",
                        chunked_file.file_name
                    );

                    if let Err(error) = fs::write(uploaded_file, []) {
                        error!(
                            "Could not write datamap for {:?}, {error:?}",
                            chunked_file.head_chunk_address
                        );
                    }
                }
            }
        }

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
    /// verified
    pub(crate) fn verified_files(&self) -> &Vec<(OsString, ChunkAddress)> {
        &self.verified_files
    }

    /// Return the filename of unverified_files.
    pub(crate) fn unverified_files(&self) -> Vec<&OsString> {
        self.chunks
            .values()
            .map(|chunked_file| &chunked_file.file_name)
            .collect()
    }

    /// Return the filename and the file's Xor address if all their chunks has been marked as
    /// verified
    pub(crate) fn already_put_chunks(
        &mut self,
        files_path: &Path,
        make_files_public: bool,
    ) -> Result<Vec<(XorName, PathBuf)>> {
        self.chunk_path(files_path, false, make_files_public)?;
        Ok(self.get_chunks())
    }

    // Try to read the chunks from `file_chunks_dir`
    // Returns the ChunkedFile if the metadata file exists
    // file_chunks_dir: artifacts_dir/path_xor
    // path_xor: Used during logging and is returned
    // original_file_name: Used to create ChunkedFile
    fn read_file_chunks_dir(
        file_chunks_dir: PathBuf,
        path_xor: &PathXorName,
        original_file_name: OsString,
    ) -> Option<(PathXorName, ChunkedFile)> {
        let mut file_chunk_address: Option<ChunkAddress> = None;
        let mut data_map: Option<Bytes> = None;
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
    /// Returning (head_chunk_address, Option<datamap_bytes>)
    fn try_read_metadata(path: &Path) -> Option<(ChunkAddress, Option<Bytes>)> {
        let metadata = fs::read(path)
            .map_err(|err| error!("Failed to read metadata with err {err:?}"))
            .ok()?;
        // head chunk address and the final datamap contents if a datamap exists for this file
        let metadata: (ChunkAddress, Option<Bytes>) = rmp_serde::from_slice(&metadata)
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
    use sn_protocol::test_utils::assert_list_eq;
    use tempfile::TempDir;

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
        manager.mark_completed(vec![chunk].into_iter());

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
        let (path_xor_from_dir, chunked_file_from_dir) =
            ChunkManager::read_file_chunks_dir(file_chunks_dir, &path_xor, chunked_file.file_name)
                .expect("Folder and metadata should be present");
        assert_eq!(chunked_file_from_dir.chunks.len(), total_chunks - 1);
        assert_eq!(chunked_file_from_dir.head_chunk_address, file_xor_addr);
        assert_eq!(path_xor_from_dir, path_xor);

        // 2. file should not be marked as verified
        assert!(manager.verified_files.is_empty());

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

        manager.mark_completed_all();

        // all 5 files should be marked as verified
        assert_eq!(manager.verified_files.len(), 5);

        // all 5 folders should exist
        for (path_xor, chunked_file) in manager_clone.chunks.iter() {
            let file_chunks_dir = manager_clone.artifacts_dir.join(path_xor.0.clone());
            let (path_xor_from_dir, chunked_file_from_dir) = ChunkManager::read_file_chunks_dir(
                file_chunks_dir,
                path_xor,
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
        assert_eq!(manager.verified_files, new_manager.verified_files);

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
        manager.mark_completed([removed_chunk].into_iter());
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

        // 2. files should not be added to verified
        assert_eq!(new_manager.verified_files.len(), 0);

        Ok(())
    }

    #[test]
    fn mark_all_and_resume() -> Result<()> {
        let _log_guards = LogBuilder::init_single_threaded_tokio_test("chunk_manager");
        let (_tmp_dir, mut manager, root_dir, random_files_dir) = init_manager()?;

        let _ = create_random_files(&random_files_dir, 5, 5)?;
        manager.chunk_path(&random_files_dir, true, true)?;
        manager.mark_completed_all();

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

        // 3. make sure the files are added to verified list
        assert_eq!(new_manager.verified_files.len(), 5);

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
