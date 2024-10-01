mod common;

use crate::common::{evm_network_from_env, evm_wallet_from_env_or_default};
use autonomi::Client;
use std::time::Duration;
use tokio::time::sleep;

#[cfg(feature = "files")]
#[tokio::test]
async fn file() -> Result<(), Box<dyn std::error::Error>> {
    common::enable_logging();

    let network = evm_network_from_env();
    let mut client = Client::connect(&[]).await.unwrap();
    let wallet = evm_wallet_from_env_or_default(network);

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

#[cfg(all(feature = "vault", feature = "files"))]
#[tokio::test]
async fn file_into_vault() -> eyre::Result<()> {
    common::enable_logging();

    let network = evm_network_from_env();

    let mut client = Client::connect(&[]).await?;
    let mut wallet = evm_wallet_from_env_or_default(network);
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
