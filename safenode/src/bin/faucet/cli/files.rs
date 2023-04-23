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
use std::{fs, path::PathBuf};
use walkdir::WalkDir;
use xor_name::XorName;

#[derive(Parser, Debug)]
pub enum FilesCmds {
    Upload {
        /// The location of the files to upload.
        #[clap(name = "path", value_name = "DIRECTORY")]
        path: PathBuf,
    },
    Download {
        /// The location of the file names stored
        /// when uploading files.
        #[clap(name = "file-names-path", value_name = "DIRECTORY")]
        file_names_path: PathBuf,
    },
}

pub(crate) async fn files_cmds(cmds: FilesCmds, client: Client) -> Result<()> {
    let file_api: Files = Files::new(client);
    match cmds {
        FilesCmds::Upload { path } => upload_files(path, &file_api).await?,
        FilesCmds::Download { file_names_path } => {
            download_files(file_names_path, &file_api).await?
        }
    };
    Ok(())
}

async fn upload_files(files_path: PathBuf, file_api: &Files) -> Result<()> {
    // The input files_path has to be a dir
    let file_names_path = files_path.join("uploaded_files");
    let mut chunks_to_fetch = Vec::new();

    for entry in WalkDir::new(files_path).into_iter().flatten() {
        if entry.file_type().is_file() {
            let file = fs::read(entry.path())?;
            let bytes = Bytes::from(file);
            let file_name = entry.file_name();

            println!("Storing file {file_name:?} of {} bytes..", bytes.len());

            match file_api.upload(bytes).await {
                Ok(address) => {
                    println!("Successfully stored file to {address:?}");
                    chunks_to_fetch.push(*address.name());
                }
                Err(error) => {
                    panic!(
                        "Did not store file {file_name:?} to all nodes in the close group! {error}"
                    )
                }
            };
        }
    }

    let content = bincode::serialize(&chunks_to_fetch)?;
    tokio::fs::create_dir_all(file_names_path.as_path()).await?;
    let file_names_path = file_names_path.join("file_names.txt");
    println!("Writing {} bytes to {file_names_path:?}", content.len());
    fs::write(file_names_path, content)?;

    Ok(())
}

async fn download_files(file_names_dir: PathBuf, file_api: &Files) -> Result<()> {
    for entry in WalkDir::new(file_names_dir).into_iter().flatten() {
        if entry.file_type().is_file() {
            let file = fs::read(entry.path())?;
            let bytes = Bytes::from(file);
            let file_name = entry.file_name();

            println!(
                "Loading file xornames from {file_name:?} with length {:?}",
                bytes.len()
            );
            let chunks_to_fetch: Vec<XorName> = bincode::deserialize(&bytes)?;

            if chunks_to_fetch.is_empty() {
                println!("No files to download!");
            }
            for xorname in chunks_to_fetch.iter() {
                println!("Downloading file {xorname:?}");
                match file_api.read_bytes(ChunkAddress::new(*xorname)).await {
                    Ok(bytes) => {
                        println!("Successfully got file {xorname} of {} bytes!", bytes.len());
                    }
                    Err(error) => {
                        panic!("Did not get file {xorname:?} from the network! {error}")
                    }
                };
            }
        }
    }

    Ok(())
}
