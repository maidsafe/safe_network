// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::get_progress_bar;
use autonomi::{
    client::{
        address::str_to_addr,
        files::{archive::PrivateArchiveAccess, archive_public::ArchiveAddr},
    },
    Client,
};
use color_eyre::{
    eyre::{eyre, Context, Result},
    Section,
};
use std::path::PathBuf;

pub async fn download(addr: &str, dest_path: &str, client: &mut Client) -> Result<()> {
    let public_address = str_to_addr(addr).ok();
    let private_address = crate::user_data::get_local_private_archive_access(addr)
        .inspect_err(|e| error!("Failed to get private archive access: {e}"))
        .ok();

    match (public_address, private_address) {
        (Some(public_address), _) => download_public(addr, public_address, dest_path, client).await,
        (_, Some(private_address)) => download_private(addr, private_address, dest_path, client).await,
        _ => Err(eyre!("Failed to parse data address {addr}"))
            .with_suggestion(|| "Public addresses look like this: 0037cfa13eae4393841cbc00c3a33cade0f98b8c1f20826e5c51f8269e7b09d7")
            .with_suggestion(|| "Private addresses look like this: 1358645341480028172")
            .with_suggestion(|| "Try the `file list` command to get addresses you have access to"),
    }
}

async fn download_private(
    addr: &str,
    private_address: PrivateArchiveAccess,
    dest_path: &str,
    client: &mut Client,
) -> Result<()> {
    let archive = client
        .archive_get(private_address)
        .await
        .wrap_err("Failed to fetch data from address")?;

    let progress_bar = get_progress_bar(archive.iter().count() as u64)?;
    let mut all_errs = vec![];
    for (path, access, _meta) in archive.iter() {
        progress_bar.println(format!("Fetching file: {path:?}..."));
        let bytes = match client.data_get(access.clone()).await {
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
        info!("Successfully downloaded private data with local address: {addr}");
        println!("Successfully downloaded private data with local address: {addr}");
        Ok(())
    } else {
        let err_no = all_errs.len();
        eprintln!("{err_no} errors while downloading private data with local address: {addr}");
        eprintln!("{all_errs:#?}");
        error!("Errors while downloading private data with local address {addr}: {all_errs:#?}");
        Err(eyre!("Errors while downloading private data"))
    }
}

async fn download_public(
    addr: &str,
    address: ArchiveAddr,
    dest_path: &str,
    client: &mut Client,
) -> Result<()> {
    let archive = client
        .archive_get_public(address)
        .await
        .wrap_err("Failed to fetch data from address")?;

    let progress_bar = get_progress_bar(archive.iter().count() as u64)?;
    let mut all_errs = vec![];
    for (path, addr, _meta) in archive.iter() {
        progress_bar.println(format!("Fetching file: {path:?}..."));
        let bytes = match client.data_get_public(*addr).await {
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
        info!("Successfully downloaded data at: {addr}");
        println!("Successfully downloaded data at: {addr}");
        Ok(())
    } else {
        let err_no = all_errs.len();
        eprintln!("{err_no} errors while downloading data at: {addr}");
        eprintln!("{all_errs:#?}");
        error!("Errors while downloading data at {addr}: {all_errs:#?}");
        Err(eyre!("Errors while downloading data"))
    }
}
