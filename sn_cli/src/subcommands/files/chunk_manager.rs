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
    os::unix::prelude::OsStrExt,
    path::{Path, PathBuf},
    time::Instant,
};
use walkdir::WalkDir;
use xor_name::XorName;

// the hex encoded XorName of the entire path's bytes
#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
struct PathXorName(String);

/// Info about a file that has been chunked
pub(crate) struct ChunkedFile {
    pub file_name: OsString,
    pub file_xor_addr: XorName,
    pub chunks: Vec<(XorName, PathBuf)>,
}
pub(crate) struct ChunkManager {
    artifacts_dir: PathBuf,
    chunked_files: BTreeMap<PathXorName, ChunkedFile>,
    uploaded_files: Vec<(OsString, XorName)>,
}

impl ChunkManager {
    pub(crate) fn new(root_dir: &Path) -> Self {
        Self {
            artifacts_dir: root_dir.join("chunk_artifacts"),
            chunked_files: Default::default(),
            uploaded_files: Default::default(),
        }
    }

    /// Chunk all the files in the provided `files_path`
    /// `chunks_dir` is used to store the results of the self-encryption process
    pub(crate) fn chunk_path(&mut self, files_path: &Path) -> Result<()> {
        trace!("Starting to chunk {files_path:?} now.");
        let now = Instant::now();

        let mut to_chunk_files = BTreeMap::new();
        let files_to_chunk: Vec<(OsString, PathXorName, PathBuf)> = WalkDir::new(files_path)
            .into_iter()
            .flatten()
            .filter_map(|entry| {
                if !entry.file_type().is_file() {
                    return None;
                }
                let path_as_bytes = entry.path().as_os_str().as_bytes();
                let path_xor = XorName::from_content(path_as_bytes);
                let path_xor = hex::encode(path_xor);

                let _ = to_chunk_files.insert(path_xor.clone(), entry.clone().into_path());

                Some((
                    entry.file_name().to_owned(),
                    PathXorName(path_xor),
                    entry.into_path(),
                ))
            })
            .collect::<Vec<_>>();

        let total_files = files_to_chunk.len();
        let resume_progress_bar = get_progress_bar(total_files as u64)?;
        resume_progress_bar.println(format!(
            "Checking for chunked files to resume. {total_files} files..."
        ));

        // the (file_chunks_dir name to (original_file_name, Path to chunks to read))
        let resumed = to_chunk_files
            .par_iter()
            .filter_map(|(path_xor, files_path)| {
                // if this folder exists, and if we find chunks under this, we upload them.
                // need to check if the filename of the chunks is it's Xorname
                let file_chunks_dir = self.artifacts_dir.join(path_xor);
                let mut file_xor_addr: Option<XorName> = None;

                let chunks = WalkDir::new(file_chunks_dir)
                    .into_iter()
                    .flatten()
                    .filter_map(|entry| {
                        if !entry.file_type().is_file() {
                            return None;
                        }
                        if entry.file_name() == "metadata" {
                            let metadata = fs::read(entry.path()).ok()?;
                            let metadata: XorName = bincode::deserialize(&metadata).ok()?;
                            file_xor_addr = Some(metadata);
                            // not a chunk, so don't return
                            return None;
                        }
                        let chunk_xorname: XorName =
                            bincode::deserialize(entry.file_name().to_str()?.as_bytes()).ok()?;
                        Some((chunk_xorname, entry.into_path()))
                    })
                    .collect();
                resume_progress_bar.clone().inc(1);
                match file_xor_addr {
                    Some(file_xor_addr) => {
                        // let chunks_to_resume = chunks_to_resume.collect();
                        let original_file_name = files_path.file_name()?.to_owned();

                        Some((
                            PathXorName(path_xor.clone()),
                            ChunkedFile {
                                file_name: original_file_name.clone(),
                                file_xor_addr,
                                chunks,
                            },
                        ))
                    }
                    None => {
                        // metadata file was not present/was not read
                        None
                    }
                }
            })
            .collect::<BTreeMap<_, _>>();
        resume_progress_bar.finish_and_clear();

        let to_filter = resumed.keys().cloned().collect::<BTreeSet<_>>();
        self.chunked_files.extend(resumed);

        let total_files = files_to_chunk.len() - to_filter.len();
        if total_files == 0 {
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
                // Each file using individual dir for temp SE chunks.
            let file_chunks_dir = {
                let file_chunks_dir = artifacts_dir.join(&path_xor.0);
                match fs::create_dir_all(&file_chunks_dir) {
                    Ok(_) => file_chunks_dir,
                    Err(err) => {
                        println!("Failed to create temp folder {file_chunks_dir:?} for SE chunks with error {err:?}!");
                        // use the chunk_artifacts_dir directly; This should not fail. Resume
                        // operation is not conisdered for this failure here.
                        // We assume each file is chunked to the `path_xor`
                        artifacts_dir.clone()
                    }
                }
            };

                match Files::chunk_file(path, &file_chunks_dir) {
                    Ok((file_xor_addr, _size, chunks)) => {
                        progress_bar.clone().inc(1);
                        Some((path_xor.clone(), ChunkedFile {file_xor_addr, file_name: original_file_name.clone(), chunks}))
                    }
                    Err(err) => {
                        println!("Skipping file {path:?} as it could not be chunked: {err:?}");
                        None
                    }
                }
            })
            .collect::<BTreeMap<_, _>>();
        // write metadata
        let _ = chunked_files
            .par_iter()
            .filter_map(|(path_xor, chunked_file)| {
                let metadata_path = artifacts_dir.join(&path_xor.0).join("metadata");
                let metadata = bincode::serialize(&chunked_file.file_xor_addr).ok()?;
                let mut metadat_file = File::create(metadata_path).ok()?;
                metadat_file.write_all(&metadata).ok()?;
                error!(
                    "Failed to write metadata for PathXorName: {path_xor:?}, file_xor_addr: {:?}",
                    chunked_file.file_xor_addr
                );
                Some(())
            });

        // todo: does this still work?
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

    pub(crate) fn get_chunks(&self) -> Vec<(XorName, PathBuf)> {
        self.chunked_files
            .values()
            .flat_map(|chunked_file| &chunked_file.chunks)
            .cloned()
            .collect()
    }

    pub(crate) fn mark_finished_all(&mut self) {
        let all_chunks = self
            .chunked_files
            .values()
            .flat_map(|chunked_file| &chunked_file.chunks)
            .map(|(chunk, _)| *chunk)
            .collect::<Vec<_>>();
        self.mark_finished(all_chunks.into_iter());
    }

    pub(crate) fn mark_finished(&mut self, finished_chunks: impl Iterator<Item = XorName>) {
        let finished_chunks = finished_chunks.collect::<BTreeSet<_>>();
        // remove those files
        let _ = self
            .chunked_files
            .par_iter()
            .flat_map(|(_, chunked_file)| &chunked_file.chunks)
            .filter_map(|(chunk_xor, chunk_path)| {
                if finished_chunks.contains(chunk_xor) {
                    fs::remove_file(chunk_path)
                        .map_err(|_err| {
                            error!("Failed to remove SE chunk {chunk_xor} from {chunk_path:?}");
                        })
                        .ok()?;
                }
                Some(())
            });
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
    pub(crate) fn completed_files(&self) -> &Vec<(OsString, XorName)> {
        &self.uploaded_files
    }
}
