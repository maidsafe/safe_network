// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use bytes::Bytes;
use clap::Parser;
use color_eyre::Result;
use sn_client::{Client, Files};
use sn_protocol::storage::ChunkAddress;
use std::{
    fs,
    path::{Path, PathBuf},
};
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
        /// Name of the file to download.
        #[clap(name = "file_name")]
        file_name: Option<String>,
        /// Address of the file to download, in hex string.
        #[clap(name = "file_addr")]
        file_addr: Option<String>,
    },
}

pub(crate) async fn files_cmds(cmds: FilesCmds, client: Client, root_dir: &Path) -> Result<()> {
    let file_api: Files = Files::new(client);
    match cmds {
        FilesCmds::Upload { path } => upload_files(path, &file_api, root_dir).await?,
        FilesCmds::Download {
            file_name,
            file_addr,
        } => match (file_name, file_addr) {
            (Some(name), Some(address)) => {
                let bytes = hex::decode(address).expect("Input address is not a hex string");
                download_file(
                    &file_api,
                    &XorName(
                        bytes
                            .try_into()
                            .expect("Failed to parse XorName from hex string"),
                    ),
                    &name,
                    root_dir,
                )
                .await
            }
            _ => {
                println!("Trying to download files recorded in uploaded_files folder");
                download_files(&file_api, root_dir).await?
            }
        },
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
                println!(
                    "Skipping file {:?} as it is not valid UTF-8.",
                    entry.file_name()
                );
                continue;
            };

            println!("Storing file {file_name:?} of {} bytes..", bytes.len());

            match file_api.upload(bytes).await {
                Ok(address) => {
                    // Output address in hex string.
                    println!(
                        "Successfully stored file {:?} to {:64x}",
                        entry.file_name(),
                        address.name()
                    );
                    chunks_to_fetch.push((*address.name(), file_name));
                }
                Err(error) => {
                    println!(
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
    println!("Writing {} bytes to {file_names_path:?}", content.len());
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

            println!("Loading file names from index doc {index_doc_name:?}");
            let files_to_fetch: Vec<(XorName, String)> = bincode::deserialize(&index_doc_bytes)?;

            if files_to_fetch.is_empty() {
                println!("No files to download!");
            }
            for (xorname, file_name) in files_to_fetch.iter() {
                download_file(file_api, xorname, file_name, &download_path).await;
            }
        }
    }

    Ok(())
}

async fn download_file(
    file_api: &Files,
    xorname: &XorName,
    file_name: &String,
    download_path: &Path,
) {
    println!(
        "Downloading file {file_name:?} with address {:64x}",
        xorname
    );
    match file_api.read_bytes(ChunkAddress::new(*xorname)).await {
        Ok(bytes) => {
            println!("Successfully got file {file_name}!");
            let file_name_path = download_path.join(file_name);
            println!("Writing {} bytes to {file_name_path:?}", bytes.len());
            if let Err(err) = fs::write(file_name_path, bytes) {
                println!("Failed to create file {file_name:?} with error {err:?}");
            }
        }
        Err(error) => {
            println!("Did not get file {file_name:?} from the network! {error}")
        }
    }
}
