// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use bytes::Bytes;
use color_eyre::Result;
use serde::Deserialize;
use sn_client::protocol::storage::ChunkAddress;
use std::{ffi::OsString, path::Path};
use tracing::{error, warn};

/// Subdir for storing uploaded file into
pub const UPLOADED_FILES: &str = "uploaded_files";

/// The metadata related to file that has been uploaded.
/// This is written during upload and read during downloads.
#[derive(Clone, Debug, Deserialize)]
pub struct UploadedFile {
    pub filename: OsString,
    pub data_map: Option<Bytes>,
}

impl UploadedFile {
    /// Write an UploadedFile to a path identified by the hex of the head ChunkAddress.
    /// If you want to update the data_map to None, calling this function will overwrite the previous value.
    pub fn write(&self, root_dir: &Path, head_chunk_address: &ChunkAddress) -> Result<()> {
        let uploaded_files = root_dir.join(UPLOADED_FILES);

        if !uploaded_files.exists() {
            if let Err(error) = std::fs::create_dir_all(&uploaded_files) {
                error!("Failed to create {uploaded_files:?} because {error:?}");
            }
        }

        let uploaded_file_path = uploaded_files.join(head_chunk_address.to_hex());

        if self.data_map.is_none() {
            warn!(
                "No data-map being written for {:?} as it is empty",
                self.filename
            );
        }
        let serialized = rmp_serde::to_vec(&(&self.filename, &self.data_map)).map_err(|err| {
            error!("Failed to serialize UploadedFile");
            err
        })?;

        std::fs::write(&uploaded_file_path, serialized).map_err(|err| {
            error!(
                "Could not write UploadedFile of {:?} to {uploaded_file_path:?}",
                self.filename
            );
            err
        })?;

        Ok(())
    }

    pub fn read(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).map_err(|err| {
            error!("Error while reading the UploadedFile from {path:?}");
            err
        })?;
        let metadata = rmp_serde::from_slice(&bytes).map_err(|err| {
            error!("Error while deserializing UploadedFile for {path:?}");
            err
        })?;
        Ok(metadata)
    }
}
