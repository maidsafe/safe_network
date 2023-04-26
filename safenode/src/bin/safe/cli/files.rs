// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use safenode::{
    client::{Client, Files},
    domain::storage::ChunkAddress,
};

use bytes::Bytes;
use clap::Parser;
use eyre::Result;
use std::{
    fs,
    path::{Path, PathBuf},
};
use tracing::trace;
use walkdir::WalkDir;
use xor_name::XorName;

#[derive(Parser, Debug)]
pub enum FilesCmds {
    Upload {
        /// The location of the files to upload.
        #[clap(name = "path", value_name = "DIRECTORY")]
        path: PathBuf,
    },
    Download,
}

pub(crate) async fn files_cmds(cmds: FilesCmds, client: Client, root_dir: &Path) -> Result<()> {
    let file_api: Files = Files::new(client);
    match cmds {
        FilesCmds::Upload { path } => upload_files(path, &file_api, root_dir).await?,
        FilesCmds::Download => download_files(&file_api, root_dir).await?,
    };
    Ok(())
}

async fn upload_files(files_path: PathBuf, file_api: &Files, root_dir: &Path) -> Result<()> {
    // The input files_path has to be a dir
    let file_names_path = root_dir.join("uploaded_files");
    let mut chunks_to_fetch = Vec::new();

    for entry in WalkDir::new(files_path).into_iter().flatten() {
        if entry.file_type().is_file() {
            let file = fs::read(entry.path())?;
            let bytes = Bytes::from(file);
            let file_name = if let Some(file_name) = entry.file_name().to_str() {
                file_name.to_string()
            } else {
                print!(
                    "Skipping file {:?} as it is not valid UTF-8.",
                    entry.file_name()
                );
                continue;
            };

            trace!("Storing file {file_name:?} of {} bytes..", bytes.len());

            match file_api.upload(bytes).await {
                Ok(address) => {
                    trace!("Successfully stored file to {address:?}");
                    chunks_to_fetch.push((*address.name(), file_name));
                }
                Err(error) => {
                    trace!(
                        "Did not store file {file_name:?} to all nodes in the close group! {error}"
                    )
                }
            };
        }
    }

    let content = bincode::serialize(&chunks_to_fetch)?;
    tokio::fs::create_dir_all(file_names_path.as_path()).await?;
    let date_time = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let file_names_path = file_names_path.join(format!("file_names_{date_time}"));
    trace!("Writing {} bytes to {file_names_path:?}", content.len());
    fs::write(file_names_path, content)?;

    Ok(())
}

async fn download_files(file_api: &Files, root_dir: &Path) -> Result<()> {
    let docs_of_uploaded_files_path = root_dir.join("uploaded_files");
    let download_path = root_dir.join("downloaded_files");
    tokio::fs::create_dir_all(download_path.as_path()).await?;

    for entry in WalkDir::new(docs_of_uploaded_files_path)
        .into_iter()
        .flatten()
    {
        if entry.file_type().is_file() {
            let index_doc_bytes = Bytes::from(fs::read(entry.path())?);
            let index_doc_name = entry.file_name();

            trace!("Loading file names from index doc {index_doc_name:?}");
            let files_to_fetch: Vec<(XorName, String)> = bincode::deserialize(&index_doc_bytes)?;

            if files_to_fetch.is_empty() {
                trace!("No files to download!");
            }
            for (xorname, file_name) in files_to_fetch.iter() {
                trace!("Downloading file {file_name:?}");
                match file_api.read_bytes(ChunkAddress::new(*xorname)).await {
                    Ok(bytes) => {
                        trace!("Successfully got file {file_name}!");
                        let file_name_path = download_path.join(file_name);
                        trace!("Writing {} bytes to {file_name_path:?}", bytes.len());
                        fs::write(file_name_path, bytes)?;
                    }
                    Err(error) => {
                        trace!("Did not get file {file_name:?} from the network! {error}")
                    }
                };
            }
        }
    }

    Ok(())
}
