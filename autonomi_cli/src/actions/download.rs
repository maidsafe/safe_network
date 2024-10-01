// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use autonomi::{client::address::str_to_xorname, Client};
use color_eyre::eyre::{eyre, Context, Result};
use std::path::PathBuf;
use super::get_progress_bar;

pub async fn download(addr: &str, dest_path: &str, client: &mut Client) -> Result<()> {
    let address = str_to_xorname(addr)
        .wrap_err("Failed to parse data address")?;
    let root = client.fetch_root(address).await
        .wrap_err("Failed to fetch data from address")?;

    let progress_bar = get_progress_bar(root.map.len() as u64)?;
    let mut all_errs = vec![];
    for (path, file) in root.map {
        progress_bar.println(format!("Fetching file: {path:?}..."));
        let bytes = match client.fetch_file(&file).await {
            Ok(bytes) => bytes,
            Err(e) => {
                let err = format!("Failed to fetch file {path:?}: {e}");
                all_errs.push(err);
                continue;
            }
        };

        let path = PathBuf::from(dest_path).join(path);
        let here = PathBuf::from(".");
        let parent = path.parent().unwrap_or_else(|| &here);
        std::fs::create_dir_all(parent)?;
        std::fs::write(path, bytes)?;
        progress_bar.clone().inc(1);
    }
    progress_bar.finish_and_clear();

    if all_errs.is_empty() {
        println!("Successfully downloaded data at: {addr}");
        Ok(())
    } else {
        let err_no = all_errs.len();
        eprintln!("{err_no} errors while downloading data at: {addr}");
        eprintln!("{all_errs:#?}");
        Err(eyre!("Errors while downloading data"))
    }
}