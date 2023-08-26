// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod common;

use std::collections::BTreeMap;

use common::{get_client_and_wallet, init_logging};

use self_encryption::MIN_ENCRYPTABLE_BYTES;
use sn_client::{Client, Error as ClientError, Files, WalletClient};
use sn_dbc::{PublicAddress, Token};
use sn_networking::Error as NetworkError;
use sn_protocol::storage::{Chunk, ChunkAddress};

use assert_fs::TempDir;
use bytes::Bytes;
use eyre::Result;
use rand::{
    distributions::{Distribution, Standard},
    Rng,
};
use tokio::time::{sleep, Duration};
use xor_name::XorName;

fn random_content(client: &Client) -> Result<(Files, Bytes, ChunkAddress, Vec<Chunk>)> {
    let mut rng = rand::thread_rng();

    let random_len = rng.gen_range(MIN_ENCRYPTABLE_BYTES..1024 * MIN_ENCRYPTABLE_BYTES);
    let random_length_content: Vec<u8> =
        <Standard as Distribution<u8>>::sample_iter(Standard, &mut rng)
            .take(random_len)
            .collect();

    let files_api = Files::new(client.clone());
    let content_bytes = Bytes::from(random_length_content);
    let (file_addr, chunks) = files_api.chunk_bytes(content_bytes.clone())?;

    Ok((
        files_api,
        content_bytes,
        ChunkAddress::new(file_addr),
        chunks,
    ))
}

#[tokio::test(flavor = "multi_thread")]
async fn storage_payment_succeeds() -> Result<()> {
    init_logging();

    let paying_wallet_balance = 50_000_000_000_000_001;
    let paying_wallet_dir = TempDir::new()?;

    let (client, paying_wallet) =
        get_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;

    let balance_before = paying_wallet.balance();
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);

    // generate a random number (between 50 and 100) of random addresses
    let mut rng = rand::thread_rng();
    let random_content_addrs = (0..rng.gen_range(50..100))
        .map(|_| {
            sn_protocol::NetworkAddress::ChunkAddress(ChunkAddress::new(XorName::random(&mut rng)))
        })
        .collect::<Vec<_>>();
    println!(
        "Paying for {} random addresses...",
        random_content_addrs.len()
    );

    let (_proofs, _cost) = wallet_client
        .pay_for_storage(random_content_addrs.clone().into_iter(), true)
        .await?;

    println!("Verifying balance has been paid from the wallet...");

    let paying_wallet = wallet_client.into_wallet();
    assert!(
        paying_wallet.balance() < balance_before,
        "balance should have decreased after payment"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn storage_payment_fails_with_insufficient_money() -> Result<()> {
    init_logging();
    let wallet_original_balance = 100_000_000_000;

    let paying_wallet_dir: TempDir = TempDir::new()?;

    let (client, paying_wallet) =
        get_client_and_wallet(paying_wallet_dir.path(), wallet_original_balance).await?;
    let (files_api, content_bytes, _random_content_addrs, chunks) = random_content(&client)?;

    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);
    let subset_len = chunks.len() / 3;
    let (subset_of_transfer_outputs_map, _storage_cost) = wallet_client
        .pay_for_storage(
            chunks
                .clone()
                .into_iter()
                .take(subset_len)
                .map(|c| c.network_address()),
            true,
        )
        .await?;

    assert_eq!(subset_of_transfer_outputs_map.len(), subset_len);

    // now let's request to upload all addresses, even that we've already paid for a subset of them
    let verify_store = false;
    let res = files_api
        .upload_with_payments(content_bytes, subset_of_transfer_outputs_map, verify_store)
        .await;
    assert!(
        res.is_err(),
        "Should have failed to store as we didnt pay for everything"
    );
    Ok(())
}

// TODO: reenable
#[ignore = "Currently we do not cache the proofs in the wallet"]
#[tokio::test(flavor = "multi_thread")]
async fn storage_payment_proofs_cached_in_wallet() -> Result<()> {
    let wallet_original_balance = 100_000_000_000_000_000;
    let paying_wallet_dir: TempDir = TempDir::new()?;

    let (client, paying_wallet) =
        get_client_and_wallet(paying_wallet_dir.path(), wallet_original_balance).await?;
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
    println!("Paying for {subset_len} random addresses...",);
    let (proofs, storage_cost) = wallet_client
        .pay_for_storage(
            random_content_addrs.clone().into_iter().take(subset_len),
            true,
        )
        .await?;
    assert_eq!(proofs.len(), subset_len);

    // check we've paid only for the subset of addresses, 1 nano per addr
    let new_balance = Token::from_nano(wallet_original_balance - storage_cost.as_nano());
    println!("Verifying new balance on paying wallet is {new_balance} ...");
    let paying_wallet = wallet_client.into_wallet();
    assert_eq!(paying_wallet.balance(), new_balance);

    // let's verify payment proofs for the subset have been cached in the wallet
    assert!(random_content_addrs
        .iter()
        .take(subset_len)
        .all(|name| paying_wallet.get_payment_dbc_ids(name) == proofs.get(name)));

    // now let's request to pay for all addresses, even that we've already paid for a subset of them
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);
    let (transfer_outputs_map, storage_cost) = wallet_client
        .pay_for_storage(random_content_addrs.clone().into_iter(), false)
        .await?;
    assert_eq!(transfer_outputs_map.len(), random_content_addrs.len());

    // check we've paid only for addresses we haven't previously paid for, 1 nano per addr
    let new_balance = Token::from_nano(
        wallet_original_balance - (random_content_addrs.len() as u64 * storage_cost.as_nano()),
    );
    println!("Verifying new balance on paying wallet is now {new_balance} ...");
    let paying_wallet = wallet_client.into_wallet();
    assert_eq!(paying_wallet.balance(), new_balance);

    // let's verify payment proofs now for all addresses have been cached in the wallet
    assert!(random_content_addrs
        .iter()
        .all(|name| paying_wallet.get_payment_dbc_ids(name) == transfer_outputs_map.get(name)));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn storage_payment_chunk_upload_succeeds() -> Result<()> {
    let paying_wallet_balance = 50_000_000_000_002;
    let paying_wallet_dir = TempDir::new()?;

    let (client, paying_wallet) =
        get_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);

    let (files_api, content_bytes, content_addr, chunks) = random_content(&client)?;

    println!("Paying for {} random addresses...", chunks.len());

    let (transfer_outputs_map, _cost) = wallet_client
        .pay_for_storage(chunks.iter().map(|c| c.network_address()), true)
        .await?;

    files_api
        .upload_with_payments(content_bytes, transfer_outputs_map, true)
        .await?;

    files_api.read_bytes(content_addr).await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn storage_payment_chunk_upload_fails() -> Result<()> {
    let paying_wallet_balance = 50_000_000_000_003;
    let paying_wallet_dir = TempDir::new()?;

    let (client, paying_wallet) =
        get_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);

    let (files_api, content_bytes, content_addr, chunks) = random_content(&client)?;

    println!("Paying for {} random addresses...", chunks.len());

    let (_transfer_outputs, _cost) = wallet_client
        .pay_for_storage(chunks.iter().map(|c| c.network_address()), true)
        .await?;

    let mut no_data_payments = BTreeMap::default();
    for chunk in chunks {
        no_data_payments.insert(
            chunk.network_address(),
            vec![(
                PublicAddress::new(bls::SecretKey::random().public_key()),
                Token::from_nano(0),
            )],
        );
    }

    let bad_transfer_outputs = wallet_client
        .into_wallet()
        .local_send_storage_payment(no_data_payments, None)
        .await?;

    // invalid spends
    client
        .send(&bad_transfer_outputs.all_spend_requests, true)
        .await?;

    sleep(Duration::from_secs(5)).await;

    // let contents_payments_map = WalletClient::store_content_payments_map(
    //     contents_payment_id_map,
    //     bad_transfer_outputs.created_dbcs,
    // );

    // let contents_payments_map = wallet_client.store_content_payments_map(

    // this should fail to store as the amount paid is not enough
    files_api
        .upload_with_payments(content_bytes.clone(), wallet_client, false)
        .await?;

    assert!(matches!(
        files_api.read_bytes(content_addr).await,
        Err(ClientError::Network(NetworkError::RecordNotFound))
    ));

    Ok(())
}
