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
    collections::BTreeMap,
    fs,
    os::unix::prelude::OsStrExt,
    path::{Path, PathBuf},
    time::Instant,
};
use walkdir::WalkDir;
use xor_name::XorName;

pub(crate) struct ChunkedFile {
    pub file_name: String,
    pub chunks: Vec<(XorName, PathBuf)>,
}

/// Chunk all the files in the provided `files_path`
/// `chunks_dir` is used to store the results of the self-encryption process
pub(crate) async fn chunk_path(
    file_api: &Files,
    files_path: &Path,
    chunks_dir: &Path,
) -> Result<BTreeMap<XorName, ChunkedFile>> {
    trace!("Starting to chunk {files_path:?} now.");
    let now = Instant::now();

    let files_to_chunk = WalkDir::new(files_path)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            if !entry.file_type().is_file() {
                return None;
            }
            let path_as_bytes = entry.path().as_os_str().as_bytes();
            let _path_xor = XorName::from_content(path_as_bytes);

            if let Some(file_name) = entry.file_name().to_str() {
                Some((file_name.to_string(), entry.into_path()))
            } else {
                println!(
                    "Skipping file {:?} as it is not valid UTF-8.",
                    entry.file_name()
                );
                None
            }
        })
        .collect::<Vec<_>>();

    let total_files = files_to_chunk.len();
    let progress_bar = get_progress_bar(total_files as u64)?;
    progress_bar.println(format!("Chunking {total_files} files..."));

    let chunked_files = files_to_chunk
        .par_iter()
        .filter_map(|(file_name, path)| {
            // Each file using individual dir for temp SE chunks.
            let file_chunks_dir = {
                let file_chunks_dir = chunks_dir.join(file_name);
                match fs::create_dir_all(&file_chunks_dir) {
                    Ok(_) => file_chunks_dir,
                    Err(err) => {
                        trace!("Failed to create temp folder {file_chunks_dir:?} for SE chunks with error {err:?}!");
                        chunks_dir.to_path_buf()
                    }
                }
            };

            match file_api.chunk_file(path, &file_chunks_dir) {
                Ok((file_addr, _size, chunks)) => {
                    progress_bar.clone().inc(1);
                    Some((file_addr, ChunkedFile {file_name: file_name.clone(), chunks}))
                }
                Err(err) => {
                    println!("Skipping file {path:?} as it could not be chunked: {err:?}");
                    None
                }
            }
        })
        .collect::<BTreeMap<_, _>>();

    if chunked_files.is_empty() {
        bail!("The provided path does not contain any file. Please check your path!\nExiting...");
    }

    progress_bar.finish_and_clear();
    debug!("It took {:?} to chunk all the files", now.elapsed());

    Ok(chunked_files)
}
