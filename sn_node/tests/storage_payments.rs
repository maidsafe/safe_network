// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod common;

use crate::common::{client::get_gossip_client_and_wallet, random_content};
use assert_fs::TempDir;
use eyre::{eyre, Result};
use rand::Rng;
use sn_client::{Error as ClientError, FilesDownload, FilesUpload, WalletClient};
use sn_logging::LogBuilder;
use sn_networking::{Error as NetworkError, GetRecordError};
use sn_protocol::{
    error::Error as ProtocolError,
    storage::{ChunkAddress, RegisterAddress},
    NetworkAddress,
};
use sn_registers::Permissions;
use sn_transfers::{MainPubkey, NanoTokens, PaymentQuote};
use std::collections::BTreeMap;
use tokio::time::{sleep, Duration};
use tracing::info;
use xor_name::XorName;

#[tokio::test]
async fn storage_payment_succeeds() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("storage_payments");

    let paying_wallet_balance = 50_000_000_000_000_001;
    let paying_wallet_dir = TempDir::new()?;

    let (client, paying_wallet) =
        get_gossip_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;

    let balance_before = paying_wallet.balance();
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);

    // generate a random number (between 50 and 100) of random addresses
    let mut rng = rand::thread_rng();
    let random_content_addrs = (0..rng.gen_range(50..100))
        .map(|_| {
            sn_protocol::NetworkAddress::ChunkAddress(ChunkAddress::new(XorName::random(&mut rng)))
        })
        .collect::<Vec<_>>();
    info!(
        "Paying for {} random addresses...",
        random_content_addrs.len()
    );

    let _cost = wallet_client
        .pay_for_storage(random_content_addrs.clone().into_iter())
        .await?;

    info!("Verifying balance has been paid from the wallet...");

    let paying_wallet = wallet_client.into_wallet();
    assert!(
        paying_wallet.balance() < balance_before,
        "balance should have decreased after payment"
    );

    Ok(())
}

#[tokio::test]
async fn storage_payment_fails_with_insufficient_money() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("storage_payments");

    let wallet_original_balance = 100_000_000_000;
    let paying_wallet_dir: TempDir = TempDir::new()?;
    let chunks_dir = TempDir::new()?;

    let (client, paying_wallet) =
        get_gossip_client_and_wallet(paying_wallet_dir.path(), wallet_original_balance).await?;

    let (files_api, content_bytes, _random_content_addrs, chunks) =
        random_content(&client, paying_wallet_dir.to_path_buf(), chunks_dir.path())?;

    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);
    let subset_len = chunks.len() / 3;
    let _storage_cost = wallet_client
        .pay_for_storage(
            chunks
                .clone()
                .into_iter()
                .take(subset_len)
                .map(|(name, _)| NetworkAddress::ChunkAddress(ChunkAddress::new(name))),
        )
        .await?;

    // now let's request to upload all addresses, even that we've already paid for a subset of them
    let verify_store = false;
    let res = files_api
        .upload_test_bytes(content_bytes.clone(), verify_store)
        .await;
    assert!(
        res.is_err(),
        "Should have failed to store as we didnt pay for everything"
    );
    Ok(())
}

// TODO: reenable
#[ignore = "Currently we do not cache the proofs in the wallet"]
#[tokio::test]
async fn storage_payment_proofs_cached_in_wallet() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("storage_payments");

    let wallet_original_balance = 100_000_000_000_000_000;
    let paying_wallet_dir: TempDir = TempDir::new()?;

    let (client, paying_wallet) =
        get_gossip_client_and_wallet(paying_wallet_dir.path(), wallet_original_balance).await?;
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);

    // generate a random number (between 50 and 100) of random addresses
    let mut rng = rand::thread_rng();
    let random_content_addrs = (0..rng.gen_range(50..100))
        .map(|_| {
            sn_protocol::NetworkAddress::ChunkAddress(ChunkAddress::new(XorName::random(&mut rng)))
        })
        .collect::<Vec<_>>();

    // let's first pay only for a subset of the addresses
    let subset_len = random_content_addrs.len() / 3;
    info!("Paying for {subset_len} random addresses...",);
    let ((storage_cost, royalties_fees), _) = wallet_client
        .pay_for_storage(random_content_addrs.clone().into_iter().take(subset_len))
        .await?;

    let total_cost = storage_cost
        .checked_add(royalties_fees)
        .ok_or(eyre!("Total storage cost exceed possible token amount"))?;

    // check we've paid only for the subset of addresses, 1 nano per addr
    let new_balance = NanoTokens::from(wallet_original_balance - total_cost.as_nano());
    info!("Verifying new balance on paying wallet is {new_balance} ...");
    let paying_wallet = wallet_client.into_wallet();
    assert_eq!(paying_wallet.balance(), new_balance);

    // let's verify payment proofs for the subset have been cached in the wallet
    assert!(random_content_addrs
        .iter()
        .take(subset_len)
        .all(|name| paying_wallet
            .get_cached_payment_for_xorname(&name.as_xorname().unwrap())
            .is_some()));

    // now let's request to pay for all addresses, even that we've already paid for a subset of them
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);
    let ((storage_cost, royalties_fees), _) = wallet_client
        .pay_for_storage(random_content_addrs.clone().into_iter())
        .await?;
    let total_cost = storage_cost
        .checked_add(royalties_fees)
        .ok_or(eyre!("Total storage cost exceed possible token amount"))?;

    // check we've paid only for addresses we haven't previously paid for, 1 nano per addr
    let new_balance = NanoTokens::from(
        wallet_original_balance - (random_content_addrs.len() as u64 * total_cost.as_nano()),
    );
    println!("Verifying new balance on paying wallet is now {new_balance} ...");
    let paying_wallet = wallet_client.into_wallet();
    assert_eq!(paying_wallet.balance(), new_balance);

    // let's verify payment proofs now for all addresses have been cached in the wallet
    // assert!(random_content_addrs
    //     .iter()
    //     .all(|name| paying_wallet.get_payment_unique_pubkeys(name) == transfer_outputs_map.get(name)));

    Ok(())
}

#[tokio::test]
async fn storage_payment_chunk_upload_succeeds() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("storage_payments");

    let paying_wallet_balance = 50_000_000_000_002;
    let paying_wallet_dir = TempDir::new()?;
    let chunks_dir = TempDir::new()?;

    let (client, paying_wallet) =
        get_gossip_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);

    let (files_api, _content_bytes, file_addr, chunks) =
        random_content(&client, paying_wallet_dir.to_path_buf(), chunks_dir.path())?;

    info!("Paying for {} random addresses...", chunks.len());

    let _cost = wallet_client
        .pay_for_storage(
            chunks
                .iter()
                .map(|(name, _)| NetworkAddress::ChunkAddress(ChunkAddress::new(*name))),
        )
        .await?;

    let mut files_upload = FilesUpload::new(files_api.clone()).set_show_holders(true);
    files_upload.upload_chunks(chunks).await?;

    let mut files_download = FilesDownload::new(files_api);
    let _ = files_download.download_file(file_addr, None).await?;

    Ok(())
}

#[tokio::test]
async fn storage_payment_chunk_upload_fails_if_no_tokens_sent() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("storage_payments");

    let paying_wallet_balance = 50_000_000_000_003;
    let paying_wallet_dir = TempDir::new()?;
    let chunks_dir = TempDir::new()?;

    let (client, paying_wallet) =
        get_gossip_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);

    let (files_api, content_bytes, content_addr, chunks) =
        random_content(&client, paying_wallet_dir.to_path_buf(), chunks_dir.path())?;

    let mut no_data_payments = BTreeMap::default();
    for (chunk_name, _) in chunks.iter() {
        no_data_payments.insert(
            *chunk_name,
            (
                MainPubkey::new(bls::SecretKey::random().public_key()),
                PaymentQuote::test_dummy(*chunk_name, NanoTokens::from(0)),
            ),
        );
    }

    let _ = wallet_client
        .mut_wallet()
        .local_send_storage_payment(&no_data_payments)?;

    sleep(Duration::from_secs(5)).await;

    files_api
        .upload_test_bytes(content_bytes.clone(), false)
        .await?;

    info!("Reading {content_addr:?} expected to fail");
    let mut files_download = FilesDownload::new(files_api);
    assert!(
        matches!(
            files_download.download_file(content_addr, None).await,
            Err(ClientError::Network(NetworkError::GetRecordError(
                GetRecordError::RecordNotFound
            )))
        ),
        "read bytes should fail as we didn't store them"
    );

    Ok(())
}

#[tokio::test]
async fn storage_payment_register_creation_succeeds() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("storage_payments");

    let paying_wallet_balance = 65_000_000_000;
    let paying_wallet_dir = TempDir::new()?;

    let (client, paying_wallet) =
        get_gossip_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);

    let mut rng = rand::thread_rng();
    let xor_name = XorName::random(&mut rng);
    let address = RegisterAddress::new(xor_name, client.signer_pk());
    let net_addr = NetworkAddress::from_register_address(address);
    info!("Paying for random Register address {net_addr:?} ...");

    let _cost = wallet_client
        .pay_for_storage(std::iter::once(net_addr))
        .await?;

    let (mut register, _cost, _royalties_fees) = client
        .create_and_pay_for_register(
            xor_name,
            &mut wallet_client,
            true,
            Permissions::new_owner_only(),
        )
        .await?;

    let retrieved_reg = client.get_register(address).await?;

    assert_eq!(register.read(), retrieved_reg.read());

    let random_entry = rng.gen::<[u8; 32]>().to_vec();

    register.write(&random_entry)?;
    register.sync(&mut wallet_client, true).await?;

    let retrieved_reg = client.get_register(address).await?;

    assert_eq!(retrieved_reg.read().iter().next().unwrap().1, random_entry);

    Ok(())
}

#[tokio::test]
#[ignore = "Test currently invalid as we always try to pay and upload registers if none found... need to check if this test is valid"]
async fn storage_payment_register_creation_and_mutation_fails() -> Result<()> {
    let _log_guards = LogBuilder::init_single_threaded_tokio_test("storage_payments");

    let paying_wallet_balance = 55_000_000_005;
    let paying_wallet_dir = TempDir::new()?;

    let (client, paying_wallet) =
        get_gossip_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);

    let mut rng = rand::thread_rng();
    let xor_name = XorName::random(&mut rng);
    let address = RegisterAddress::new(xor_name, client.signer_pk());
    let net_address =
        NetworkAddress::RegisterAddress(RegisterAddress::new(xor_name, client.signer_pk()));

    let mut no_data_payments = BTreeMap::default();
    no_data_payments.insert(
        net_address
            .as_xorname()
            .expect("RegisterAddress should convert to XorName"),
        (
            MainPubkey::new(bls::SecretKey::random().public_key()),
            PaymentQuote::test_dummy(xor_name, NanoTokens::from(0)),
        ),
    );

    let _ = wallet_client
        .mut_wallet()
        .local_send_storage_payment(&no_data_payments)?;

    // this should fail to store as the amount paid is not enough
    let (mut register, _cost, _royalties_fees) = client
        .create_and_pay_for_register(
            xor_name,
            &mut wallet_client,
            false,
            Permissions::new_owner_only(),
        )
        .await?;

    sleep(Duration::from_secs(5)).await;
    assert!(matches!(
        client.get_register(address).await,
        Err(ClientError::Protocol(ProtocolError::RegisterNotFound(addr))) if *addr == address
    ));

    let random_entry = rng.gen::<[u8; 32]>().to_vec();
    register.write(&random_entry)?;

    sleep(Duration::from_secs(5)).await;
    assert!(matches!(
    register.sync(&mut wallet_client, false).await,
            Err(ClientError::Protocol(ProtocolError::RegisterNotFound(addr))) if *addr == address
        ));

    Ok(())
}
