// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#![cfg(all(feature = "files", feature = "fs"))]

mod common;

use autonomi::Client;
use common::peers_from_env;
use eyre::Result;
use sn_logging::LogBuilder;
use std::time::Duration;
use test_utils::evm::get_funded_wallet;
use tokio::time::sleep;

#[tokio::test]
async fn file() -> Result<()> {
    let _log_appender_guard = LogBuilder::init_single_threaded_tokio_test("file", false);

    let mut client = Client::connect(&peers_from_env()?).await?;
    let wallet = get_funded_wallet();

    let (root, addr) = client
        .upload_from_dir("tests/file/test_dir".into(), &wallet)
        .await?;

    sleep(Duration::from_secs(10)).await;

    let root_fetched = client.fetch_root(addr).await?;

    assert_eq!(
        root.map, root_fetched.map,
        "root fetched should match root put"
    );

    Ok(())
}

#[cfg(feature = "vault")]
#[tokio::test]
async fn file_into_vault() -> Result<()> {
    let _log_appender_guard = LogBuilder::init_single_threaded_tokio_test("file", false);

    let mut client = Client::connect(&peers_from_env()?).await?;
    let mut wallet = get_funded_wallet();
    let client_sk = bls::SecretKey::random();

    let (root, addr) = client
        .upload_from_dir("tests/file/test_dir".into(), &wallet)
        .await?;
    sleep(Duration::from_secs(2)).await;

    let root_fetched = client.fetch_root(addr).await?;
    client
        .write_bytes_to_vault(root.into_bytes()?, &mut wallet, &client_sk)
        .await?;

    assert_eq!(
        root.map, root_fetched.map,
        "root fetched should match root put"
    );

    // now assert over the stored account packet
    let new_client = Client::connect(&[]).await?;

    if let Some(ap) = new_client.fetch_and_decrypt_vault(&client_sk).await? {
        let ap_root_fetched = autonomi::client::files::Root::from_bytes(ap)?;

        assert_eq!(
            root.map, ap_root_fetched.map,
            "root fetched should match root put"
        );
    } else {
        eyre::bail!("No account packet found");
    }

    Ok(())
}
