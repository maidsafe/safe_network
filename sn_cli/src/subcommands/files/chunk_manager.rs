// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::subcommands::files::get_progress_bar;
use color_eyre::{eyre::bail, Result};
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use sn_client::Files;
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

// The unique hex encoded hash(path)
// This allows us to uniquely identify if a file has been chuked or not.
// An alternative to use instead of filename as it might not be unique
#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
struct PathXorName(String);

impl PathXorName {
    fn new(path: &Path) -> PathXorName {
        // we just need an unique value per path, thus we don't have to mind between the
        // [u8]/[u16]
        let path_as_lossy_str = path.as_os_str().to_string_lossy();
        let path_xor = XorName::from_content(path_as_lossy_str.as_bytes());
        PathXorName(hex::encode(path_xor))
    }
}

/// Info about a file that has been chunked
#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub(crate) struct ChunkedFile {
    pub file_name: OsString,
    pub file_xor_addr: XorName,
    pub chunks: Vec<(XorName, PathBuf)>,
}

/// Manages the chunking process by resuming pre-chunked files and chunking any
/// file that has not been chunked yet.
#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub(crate) struct ChunkManager {
    artifacts_dir: PathBuf,
    chunked_files: BTreeMap<PathXorName, ChunkedFile>,
    uploaded_files: Vec<(OsString, XorName)>,
    resumed_chunk_count: usize,
}

impl ChunkManager {
    // Provide the root_dir. The function creates a sub-directory to store the SE chunks
    pub(crate) fn new(root_dir: &Path) -> Self {
        Self {
            artifacts_dir: root_dir.join(CHUNK_ARTIFACTS_DIR),
            chunked_files: Default::default(),
            uploaded_files: Default::default(),
            resumed_chunk_count: 0,
        }
    }

    /// Chunk all the files in the provided `files_path`
    pub(crate) fn chunk_path(&mut self, files_path: &Path) -> Result<()> {
        trace!("Starting to chunk {files_path:?} now.");
        let now = Instant::now();

        let files_to_chunk: Vec<(OsString, PathXorName, PathBuf)> = WalkDir::new(files_path)
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                if !entry.file_type().is_file() {
                    return None;
                }
                let path_xor = PathXorName::new(entry.path());
                info!(
                    "Added file {:?} with path_xor: {path_xor:?} to be chunked/resumed",
                    entry.path()
                );

                Some((entry.file_name().to_owned(), path_xor, entry.into_path()))
            })
            .collect::<Vec<_>>();

        // resume chunks if any
        let resumed = Self::resume_path(&files_to_chunk, &self.artifacts_dir)?;
        self.resumed_chunk_count = resumed
            .values()
            .flat_map(|chunked_file| &chunked_file.chunks)
            .count();
        let to_filter = resumed.keys().cloned().collect::<BTreeSet<_>>();
        self.chunked_files.extend(resumed);

        // we don't care if we have partial chunks as they might have been marked as completed
        let total_files = files_to_chunk.len() - to_filter.len();
        if total_files == 0 {
            debug!(
                "All files_to_chunk ({:?}) were resumed. Returning the resumed chunks.",
                files_to_chunk.len()
            );
            debug!("It took {:?} to resume all the files", now.elapsed());
            // no more files to chunk
            return Ok(());
        }
        let progress_bar = get_progress_bar(total_files as u64)?;
        progress_bar.println(format!("Chunking {total_files} files..."));

        let artifacts_dir = &self.artifacts_dir;
        let chunked_files = files_to_chunk
            .par_iter()
            // filter out all the resumed ones
            .filter(|(_,path_xor, _)| !to_filter.contains(path_xor))
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
                            artifacts_dir.clone()
                        }
                    }
                };

                match Files::chunk_file(path, &file_chunks_dir) {
                    Ok((file_xor_addr, size, chunks)) => {
                        progress_bar.clone().inc(1);
                        debug!("Chunked {original_file_name:?} with {path_xor:?} into file's XorName: {file_xor_addr:?} of size {size}, and chunks len: {}", chunks.len());

                        Some((path_xor.clone(), ChunkedFile {file_xor_addr, file_name: original_file_name.clone(), chunks}))
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
            "Out of total files_to_chunk {}, we have resumed {} files and chunked {} files",
            files_to_chunk.len(),
            to_filter.len(),
            chunked_files.len()
        );

        // write metadata
        let _ = chunked_files
            .par_iter()
            .filter_map(|(path_xor, chunked_file)| {
                let metadata_path = artifacts_dir.join(&path_xor.0).join("metadata");
                let metadata = bincode::serialize(&chunked_file.file_xor_addr)
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

        if chunked_files.is_empty() {
            bail!(
                "The provided path does not contain any file. Please check your path!\nExiting..."
            );
        }

        progress_bar.finish_and_clear();
        debug!("It took {:?} to chunk all the files", now.elapsed());
        self.chunked_files.extend(chunked_files);

        Ok(())
    }

    // Try to resume all the given files.
    // Return the set of chunked_files if that we were able to resume.
    fn resume_path(
        files_to_chunk: &Vec<(OsString, PathXorName, PathBuf)>,
        artifacts_dir: &Path,
    ) -> Result<BTreeMap<PathXorName, ChunkedFile>> {
        let total_files = files_to_chunk.len();
        let resume_progress_bar = get_progress_bar(total_files as u64)?;
        resume_progress_bar.println(format!(
            "Checking for chunked files to resume. {total_files} files..."
        ));

        let resumed = files_to_chunk
            .par_iter()
            .filter_map(|(_, path_xor, files_path)| {
                // if this folder exists, and if we find chunks under this, we upload them.
                let file_chunks_dir = artifacts_dir.join(&path_xor.0);
                if !file_chunks_dir.exists() {
                    return None;
                }
                let (chunks, file_xor_addr) =
                    Self::read_file_chunks_dir(file_chunks_dir, path_xor.clone());

                resume_progress_bar.clone().inc(1);
                match file_xor_addr {
                    Some(file_xor_addr) => {
                        let original_file_name = files_path.file_name()?.to_owned();
                        debug!(
                            "Resuming {} chunks for file {original_file_name:?} and with file_xor_addr {file_xor_addr:?}/{path_xor:?}",
                            chunks.len()
                        );

                        Some((
                            path_xor.clone(),
                            ChunkedFile {
                                file_name: original_file_name.clone(),
                                file_xor_addr,
                                chunks,
                            },
                        ))
                    }
                    None => {
                        error!("Metadata file was not present for {path_xor:?}");
                        // metadata file was not present/was not read
                        None
                    }
                }
            })
            .collect::<BTreeMap<_, _>>();
        resume_progress_bar.finish_and_clear();
        Ok(resumed)
    }

    /// Get all the chunk name and their path
    pub(crate) fn get_chunks(&self) -> Vec<(XorName, PathBuf)> {
        self.chunked_files
            .values()
            .flat_map(|chunked_file| &chunked_file.chunks)
            .cloned()
            .collect()
    }

    /// Mark all the chunks as completed and remove them from the chunk_artifacts_dir
    /// Thus they cannot be resumed if we try to call `chunk_path()` again.
    pub(crate) fn mark_finished_all(&mut self) {
        let all_chunks = self
            .chunked_files
            .values()
            .flat_map(|chunked_file| &chunked_file.chunks)
            .map(|(chunk, _)| *chunk)
            .collect::<Vec<_>>();
        self.mark_finished(all_chunks.into_iter());
    }

    /// Mark a set of chunks as completed and remove them from the chunk_artifacts_dir
    /// These chunks cannot be resumed if we try to call `chunk_path()` again.
    pub(crate) fn mark_finished(&mut self, finished_chunks: impl Iterator<Item = XorName>) {
        let finished_chunks = finished_chunks.collect::<BTreeSet<_>>();
        // remove those files
        let _ = self
            .chunked_files
            .par_iter()
            .flat_map(|(_, chunked_file)| &chunked_file.chunks)
            .filter_map(|(chunk_xor, chunk_path)| {
                if finished_chunks.contains(chunk_xor) {
                    debug!("removing {chunk_xor:?} at {chunk_path:?} as it is marked as finished");
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
        self.chunked_files
            .iter_mut()
            .for_each(|(path_xor, chunked_file)| {
                chunked_file
                    .chunks
                    // if chunk is part of finished_chunk, return false to remove it
                    .retain(|(chunk_xor, _)| !finished_chunks.contains(chunk_xor));
                if chunked_file.chunks.is_empty() {
                    entire_file_is_done.insert(path_xor.clone());
                }
            });

        if !entire_file_is_done.is_empty() {
            let artifacts_dir = &self.artifacts_dir;

            for path_xor in &entire_file_is_done {
                debug!("Removing {path_xor:?} as the entire file is done");
                if let Some(chunked_file) = self.chunked_files.remove(path_xor) {
                    self.uploaded_files
                        .push((chunked_file.file_name, chunked_file.file_xor_addr));
                }
            }
            entire_file_is_done.par_iter().for_each(|path_xor| {
                let file_chunks_dir = artifacts_dir.join(&path_xor.0);
                if let Err(err) = fs::remove_dir_all(&file_chunks_dir) {
                    error!("Error while cleaning up {file_chunks_dir:?}, err: {err:?}");
                }
            })
        }
    }

    /// Return the filename and the file's Xor address if all their chunks has been marked as
    /// completed
    pub(crate) fn completed_files(&self) -> &Vec<(OsString, XorName)> {
        &self.uploaded_files
    }

    // Try to read the chunks from `file_chunks_dir`
    // Also returns the original file's XorName stored inside the metadata file.
    fn read_file_chunks_dir(
        file_chunks_dir: PathBuf,
        path_xor: PathXorName,
    ) -> (Vec<(XorName, PathBuf)>, Option<XorName>) {
        let mut file_xor_addr: Option<XorName> = None;
        debug!("Trying to resume {path_xor:?} as the file_chunks_dir exists");

        let chunks = WalkDir::new(file_chunks_dir)
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                if !entry.file_type().is_file() {
                    return None;
                }
                if entry.file_name() == "metadata" {
                    if let Some(metadata) = Self::try_read_metadata(entry.path()) {
                        file_xor_addr = Some(metadata);
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
            .collect();
        (chunks, file_xor_addr)
    }

    // Try to read the metadata file
    fn try_read_metadata(path: &Path) -> Option<XorName> {
        let metadata = fs::read(path)
            .map_err(|err| error!("Failed to read metadata with err {err:?}"))
            .ok()?;
        let metadata: XorName = bincode::deserialize(&metadata)
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

    #[cfg(test)]
    fn hex_enocode_xorname(xorname: XorName) -> String {
        hex::encode(xorname)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use color_eyre::{eyre::eyre, Result};
    use rand::{thread_rng, Rng};
    use rayon::prelude::IntoParallelIterator;
    use sn_logging::LogBuilder;
    use sn_protocol::test_utils::assert_lists;

    #[test]
    fn chunked_files_should_be_written_to_artifacts_dir() -> Result<()> {
        let _log_guards = LogBuilder::init_single_threaded_tokio_test("chunk_manager");

        let tmp_dir = tempfile::tempdir()?;
        let mut manager = ChunkManager::new(tmp_dir.path());
        let random_file = create_random_files(tmp_dir.path(), 1, 1)?.remove(0);
        manager.chunk_path(&random_file)?;

        let chunks = manager.get_chunks();
        // 1mb file produces a chunk of size 1
        assert_eq!(chunks.len(), 4);

        // check the chunks inside the temp dir
        let artifacts_dir = tmp_dir.path().join(CHUNK_ARTIFACTS_DIR);

        let n_folders = WalkDir::new(&artifacts_dir)
            .into_iter()
            .flatten()
            .filter(|entry| entry.file_type().is_dir() && entry.path() != artifacts_dir)
            .count();
        // 1 dir per file
        assert_eq!(n_folders, 1);

        let n_files = WalkDir::new(&artifacts_dir)
            .into_iter()
            .flatten()
            .filter(|entry| entry.file_type().is_file())
            .count();

        // 1 per chunk and 1 metadata file
        assert_eq!(n_files, chunks.len() + 1);

        // verify the metadata
        let mut file_xor_addr_from_metadata = None;
        for entry in WalkDir::new(&artifacts_dir).into_iter().flatten() {
            if entry.file_type().is_file() && entry.file_name() == "metadata" {
                file_xor_addr_from_metadata = ChunkManager::try_read_metadata(entry.path());
            }
        }
        let file_xor_addr_from_metadata =
            file_xor_addr_from_metadata.expect("The metadata file should be presesnt");
        let file_xor_addr = manager
            .chunked_files
            .values()
            .next()
            .expect("We must have 1 file here")
            .file_xor_addr;
        assert_eq!(file_xor_addr_from_metadata, file_xor_addr);

        // make sure the chunked file's name is the XorName of that chunk
        let chunk_xornames = manager
            .chunked_files
            .values()
            .next()
            .expect("We must have 1 file here")
            .chunks
            .iter()
            .map(|(xor_name, _)| *xor_name)
            .collect::<BTreeSet<_>>();
        for entry in WalkDir::new(&artifacts_dir).into_iter().flatten() {
            let file_name = entry.file_name();
            if entry.file_type().is_file() && file_name != "metadata" {
                let chunk_xorname_from_filename =
                    ChunkManager::hex_decode_xorname(file_name.to_str().unwrap())
                        .expect("Failed to get xorname from hex encoded file_name");
                assert!(chunk_xornames.contains(&chunk_xorname_from_filename));
            }
        }

        Ok(())
    }

    #[test]
    fn all_chunks_should_be_resumed_if_none_are_marked_as_finished() -> Result<()> {
        let _log_guards = LogBuilder::init_single_threaded_tokio_test("chunk_manager");

        let tmp_dir = tempfile::tempdir()?;
        let random_files_dir = tmp_dir.path().join("random_files");
        let root_dir = tmp_dir.path().join("root_dir");
        fs::create_dir_all(&random_files_dir)?;
        fs::create_dir_all(&root_dir)?;

        let _ = create_random_files(&random_files_dir, 5, 5)?;

        let mut manager = ChunkManager::new(&root_dir);
        manager.chunk_path(&random_files_dir)?;

        let mut new_manager = ChunkManager::new(&root_dir);
        new_manager.chunk_path(&random_files_dir)?;

        let original_chunk_count = manager
            .chunked_files
            .values()
            .flat_map(|chunked_file| &chunked_file.chunks)
            .count();
        assert_eq!(manager.resumed_chunk_count, 0);
        assert_eq!(new_manager.resumed_chunk_count, original_chunk_count);

        // sort the chunks as they might have been inserted in an random order
        manager
            .chunked_files
            .values_mut()
            .for_each(|chunked_file| chunked_file.chunks.sort());
        new_manager
            .chunked_files
            .values_mut()
            .for_each(|chunked_file| chunked_file.chunks.sort());
        assert_eq!(manager.chunked_files, new_manager.chunked_files);
        assert_eq!(manager.uploaded_files, new_manager.uploaded_files);

        Ok(())
    }

    #[test]
    fn not_all_chunks_should_be_resumed_if_marked_as_finished() -> Result<()> {
        let _log_guards = LogBuilder::init_single_threaded_tokio_test("chunk_manager");

        let tmp_dir = tempfile::tempdir()?;
        let random_files_dir = tmp_dir.path().join("random_files");
        let root_dir = tmp_dir.path().join("root_dir");
        fs::create_dir_all(&random_files_dir)?;
        fs::create_dir_all(&root_dir)?;

        let _ = create_random_files(&random_files_dir, 5, 5)?;

        let mut manager = ChunkManager::new(&root_dir);
        manager.chunk_path(&random_files_dir)?;
        let original_chunk_count = manager
            .chunked_files
            .values()
            .flat_map(|chunked_file| &chunked_file.chunks)
            .count();

        // mark a chunk as finished
        let removed_chunk = manager
            .chunked_files
            .values()
            .next()
            .expect("Atleast 1 file should be present")
            .chunks[0]
            .0;
        manager.mark_finished([removed_chunk].into_iter());

        let mut new_manager = ChunkManager::new(&root_dir);
        new_manager.chunk_path(&random_files_dir)?;

        assert_eq!(manager.resumed_chunk_count, 0);
        assert_eq!(new_manager.resumed_chunk_count, original_chunk_count - 1);

        Ok(())
    }

    #[test]
    fn mark_finished_should_remove_chunk_from_artifacts_dir() -> Result<()> {
        let _log_guards = LogBuilder::init_single_threaded_tokio_test("chunk_manager");

        let tmp_dir = tempfile::tempdir()?;
        let random_files_dir = tmp_dir.path().join("random_files");
        let root_dir = tmp_dir.path().join("root_dir");
        let artifacts_dir = root_dir.join(CHUNK_ARTIFACTS_DIR);
        fs::create_dir_all(&root_dir)?;
        fs::create_dir_all(&random_files_dir)?;

        let _ = create_random_files(&random_files_dir, 1, 5)?;

        let mut manager = ChunkManager::new(&root_dir);
        manager.chunk_path(&random_files_dir)?;

        // make sure the dir and the struct data match
        let old_chunks = manager
            .chunked_files
            .values()
            .next()
            .expect("We must have 1 file here")
            .chunks
            .iter()
            .map(|(xor_name, _)| *xor_name)
            .collect::<BTreeSet<_>>();
        let mut old_chunks_from_dir = BTreeSet::new();
        for entry in WalkDir::new(&artifacts_dir).into_iter().flatten() {
            let file_name = entry.file_name();
            if entry.file_type().is_file() && file_name != "metadata" {
                let chunk_xorname_from_filename =
                    ChunkManager::hex_decode_xorname(file_name.to_str().unwrap())
                        .expect("Failed to get xorname from hex encoded file_name");
                old_chunks_from_dir.insert(chunk_xorname_from_filename);
            }
        }
        assert_lists(old_chunks, old_chunks_from_dir);

        // mark a chunk as finished
        let removed_chunk = manager
            .chunked_files
            .values()
            .next()
            .expect("Atleast 1 file should be present")
            .chunks[0]
            .0;
        manager.mark_finished([removed_chunk].into_iter());

        // compare the dir and struct data
        let new_chunks = manager
            .chunked_files
            .values()
            .next()
            .expect("We must have 1 file here")
            .chunks
            .iter()
            .map(|(xor_name, _)| *xor_name)
            .collect::<BTreeSet<_>>();
        let mut new_chunks_from_dir = BTreeSet::new();
        for entry in WalkDir::new(&artifacts_dir).into_iter().flatten() {
            let file_name = entry.file_name();
            if entry.file_type().is_file() && file_name != "metadata" {
                let chunk_xorname_from_filename =
                    ChunkManager::hex_decode_xorname(file_name.to_str().unwrap())
                        .expect("Failed to get xorname from hex encoded file_name");
                new_chunks_from_dir.insert(chunk_xorname_from_filename);
            }
        }
        assert!(!new_chunks.contains(&removed_chunk));
        assert!(!new_chunks_from_dir.contains(&removed_chunk));
        assert_lists(new_chunks, new_chunks_from_dir);

        Ok(())
    }

    #[test]
    fn mark_finished_all_should_remove_all_the_chunks() -> Result<()> {
        let _log_guards = LogBuilder::init_single_threaded_tokio_test("chunk_manager");

        let tmp_dir = tempfile::tempdir()?;
        let random_files_dir = tmp_dir.path().join("random_files");
        let root_dir = tmp_dir.path().join("root_dir");
        let artifacts_dir = root_dir.join(CHUNK_ARTIFACTS_DIR);
        fs::create_dir_all(&root_dir)?;
        fs::create_dir_all(&random_files_dir)?;

        let mut random_file = create_random_files(&random_files_dir, 1, 5)?;
        let random_file = random_file.remove(0);

        let mut manager = ChunkManager::new(&root_dir);
        manager.chunk_path(&random_files_dir)?;

        // make sure the dir and the struct data match
        let old_chunks = manager
            .chunked_files
            .values()
            .next()
            .expect("We must have 1 file here")
            .chunks
            .iter()
            .map(|(xor_name, _)| *xor_name)
            .collect::<BTreeSet<_>>();
        let mut file_xor_addr = None;
        let mut old_chunks_from_dir = BTreeSet::new();
        for entry in WalkDir::new(&artifacts_dir).into_iter().flatten() {
            let file_name = entry.file_name();
            if entry.file_type().is_file() && file_name != "metadata" {
                let chunk_xorname_from_filename =
                    ChunkManager::hex_decode_xorname(file_name.to_str().unwrap())
                        .expect("Failed to get xorname from hex encoded file_name");
                old_chunks_from_dir.insert(chunk_xorname_from_filename);
            }

            if entry.file_type().is_file() && file_name == "metadata" {
                file_xor_addr = ChunkManager::try_read_metadata(entry.path());
            }
        }
        assert_lists(old_chunks, old_chunks_from_dir);

        // mark all as finished
        manager.mark_finished_all();

        // should be removed from struct and fs
        assert!(manager.chunked_files.values().next().is_none());
        assert!(fs::read_dir(&artifacts_dir)?.next().is_none());

        // should be added to uploaded_files
        assert_eq!(manager.uploaded_files.len(), 1);
        let uploaded = manager.uploaded_files.remove(0);
        assert_eq!(
            uploaded.0,
            random_file.file_name().expect("Not a directory")
        );
        assert_eq!(
            uploaded.1,
            file_xor_addr.expect("Metadata file should be present")
        );

        Ok(())
    }

    #[test]
    fn file_should_be_re_chunked_if_metadata_file_is_absent() -> Result<()> {
        let _log_guards = LogBuilder::init_single_threaded_tokio_test("chunk_manager");

        let tmp_dir = tempfile::tempdir()?;
        let random_files_dir = tmp_dir.path().join("random_files");
        let root_dir = tmp_dir.path().join("root_dir");
        let artifacts_dir = root_dir.join(CHUNK_ARTIFACTS_DIR);
        fs::create_dir_all(&root_dir)?;
        fs::create_dir_all(&random_files_dir)?;

        let mut random_file = create_random_files(&random_files_dir, 1, 5)?;
        let random_file = random_file.remove(0);

        let mut manager = ChunkManager::new(&root_dir);
        manager.chunk_path(&random_files_dir)?;

        let mut old_chunks_from_dir = BTreeSet::new();
        for entry in WalkDir::new(&artifacts_dir).into_iter().flatten() {
            let file_name = entry.file_name();
            if entry.file_type().is_file() && file_name != "metadata" {
                let chunk_xorname_from_filename =
                    ChunkManager::hex_decode_xorname(file_name.to_str().unwrap())
                        .expect("Failed to get xorname from hex encoded file_name");
                old_chunks_from_dir.insert(chunk_xorname_from_filename);
            }
        }

        // remove metadata file
        let path_xor = PathXorName::new(&random_file).0;
        let metadata_path = artifacts_dir.join(&path_xor).join("metadata");
        fs::remove_file(&metadata_path)?;
        // also remove a random chunk to make sure it is re-chunked
        let removed_chunk = manager
            .chunked_files
            .values()
            .next()
            .expect("We must have 1 file here")
            .chunks
            .get(0)
            .expect("We must have atleast 1 chunk")
            .0;
        fs::remove_file(
            artifacts_dir
                .join(path_xor)
                .join(ChunkManager::hex_enocode_xorname(removed_chunk)),
        )?;

        // use the same manager to chunk the path
        assert_eq!(manager.resumed_chunk_count, 0);
        manager.chunk_path(&random_files_dir)?;
        // nothing should be resumed
        assert_eq!(manager.resumed_chunk_count, 0);

        let mut new_chunks_from_dir = BTreeSet::new();
        for entry in WalkDir::new(&artifacts_dir).into_iter().flatten() {
            let file_name = entry.file_name();
            if entry.file_type().is_file() && file_name != "metadata" {
                let chunk_xorname_from_filename =
                    ChunkManager::hex_decode_xorname(file_name.to_str().unwrap())
                        .expect("Failed to get xorname from hex encoded file_name");
                new_chunks_from_dir.insert(chunk_xorname_from_filename);
            }
        }

        assert!(new_chunks_from_dir.contains(&removed_chunk));
        assert_lists(old_chunks_from_dir, new_chunks_from_dir);
        assert!(metadata_path.exists());

        Ok(())
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
