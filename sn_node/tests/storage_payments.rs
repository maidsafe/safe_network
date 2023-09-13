// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod common;

use std::collections::BTreeMap;

use common::{get_client_and_wallet, init_logging, random_content};

use sn_client::{Error as ClientError, WalletClient};
use sn_dbc::{PublicAddress, Token};
use sn_networking::Error as NetworkError;
use sn_protocol::{
    error::Error as ProtocolError,
    storage::{ChunkAddress, RegisterAddress},
    NetworkAddress,
};

use assert_fs::TempDir;
use eyre::Result;
use rand::Rng;
use tokio::time::{sleep, Duration};
use xor_name::XorName;

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

    let _cost = wallet_client
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
    let chunks_dir = TempDir::new()?;

    let (client, paying_wallet) =
        get_client_and_wallet(paying_wallet_dir.path(), wallet_original_balance).await?;

    let (files_api, content_bytes, _random_content_addrs, chunks) = random_content(
        &client,
        paying_wallet_dir.to_path_buf(),
        chunks_dir.path().to_path_buf(),
    )?;

    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);
    let subset_len = chunks.len() / 3;
    let _storage_cost = wallet_client
        .pay_for_storage(
            chunks
                .clone()
                .into_iter()
                .take(subset_len)
                .map(|(name, _)| NetworkAddress::ChunkAddress(ChunkAddress::new(name))),
            true,
        )
        .await?;

    // now let's request to upload all addresses, even that we've already paid for a subset of them
    let verify_store = false;
    let res = files_api
        .upload_with_payments(content_bytes, &wallet_client, verify_store)
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
    let storage_cost = wallet_client
        .pay_for_storage(
            random_content_addrs.clone().into_iter().take(subset_len),
            true,
        )
        .await?;

    // check we've paid only for the subset of addresses, 1 nano per addr
    let new_balance = Token::from_nano(wallet_original_balance - storage_cost.as_nano());
    println!("Verifying new balance on paying wallet is {new_balance} ...");
    let paying_wallet = wallet_client.into_wallet();
    assert_eq!(paying_wallet.balance(), new_balance);

    // let's verify payment proofs for the subset have been cached in the wallet
    assert!(random_content_addrs
        .iter()
        .take(subset_len)
        .all(|name| paying_wallet.get_payment_dbc_ids(name).is_some()));

    // now let's request to pay for all addresses, even that we've already paid for a subset of them
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);
    let storage_cost = wallet_client
        .pay_for_storage(random_content_addrs.clone().into_iter(), false)
        .await?;

    // check we've paid only for addresses we haven't previously paid for, 1 nano per addr
    let new_balance = Token::from_nano(
        wallet_original_balance - (random_content_addrs.len() as u64 * storage_cost.as_nano()),
    );
    println!("Verifying new balance on paying wallet is now {new_balance} ...");
    let paying_wallet = wallet_client.into_wallet();
    assert_eq!(paying_wallet.balance(), new_balance);

    // let's verify payment proofs now for all addresses have been cached in the wallet
    // assert!(random_content_addrs
    //     .iter()
    //     .all(|name| paying_wallet.get_payment_dbc_ids(name) == transfer_outputs_map.get(name)));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn storage_payment_chunk_upload_succeeds() -> Result<()> {
    let paying_wallet_balance = 50_000_000_000_002;
    let paying_wallet_dir = TempDir::new()?;
    let chunks_dir = TempDir::new()?;

    let (client, paying_wallet) =
        get_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);

    let (files_api, content_bytes, content_addr, chunks) = random_content(
        &client,
        paying_wallet_dir.to_path_buf(),
        chunks_dir.path().to_path_buf(),
    )?;

    println!("Paying for {} random addresses...", chunks.len());

    let _cost = wallet_client
        .pay_for_storage(
            chunks
                .iter()
                .map(|(name, _)| NetworkAddress::ChunkAddress(ChunkAddress::new(*name))),
            true,
        )
        .await?;

    files_api
        .upload_with_payments(content_bytes, &wallet_client, true)
        .await?;

    files_api.read_bytes(content_addr, None).await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn storage_payment_chunk_upload_fails() -> Result<()> {
    let paying_wallet_balance = 50_000_000_000_003;
    let paying_wallet_dir = TempDir::new()?;
    let chunks_dir = TempDir::new()?;

    let (client, paying_wallet) =
        get_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);

    let (files_api, content_bytes, content_addr, chunks) = random_content(
        &client,
        paying_wallet_dir.to_path_buf(),
        chunks_dir.path().to_path_buf(),
    )?;

    println!("Paying for {} random addresses...", chunks.len());

    let _cost = wallet_client
        .pay_for_storage(
            chunks
                .iter()
                .map(|(name, _)| NetworkAddress::ChunkAddress(ChunkAddress::new(*name))),
            true,
        )
        .await?;

    let mut no_data_payments = BTreeMap::default();
    for (chunk_name, _) in chunks.iter() {
        no_data_payments.insert(
            NetworkAddress::ChunkAddress(ChunkAddress::new(*chunk_name)),
            vec![(
                PublicAddress::new(bls::SecretKey::random().public_key()),
                Token::from_nano(0),
            )],
        );
    }

    wallet_client
        .mut_wallet()
        .local_send_storage_payment(no_data_payments, None)?;

    // invalid spends
    client.send(wallet_client.unconfirmed_txs(), true).await?;

    sleep(Duration::from_secs(5)).await;

    // this should fail to store as the amount paid is not enough
    files_api
        .upload_with_payments(content_bytes.clone(), &wallet_client, false)
        .await?;

    assert!(matches!(
        files_api.read_bytes(content_addr, None).await,
        Err(ClientError::Network(NetworkError::RecordNotFound))
    ));

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn storage_payment_register_creation_succeeds() -> Result<()> {
    let paying_wallet_balance = 65_000_000_000;
    let paying_wallet_dir = TempDir::new()?;

    let (client, paying_wallet) =
        get_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);

    println!("Paying for random Register address...");
    let mut rng = rand::thread_rng();
    let xor_name = XorName::random(&mut rng);
    let address = RegisterAddress::new(xor_name, client.signer_pk());
    let net_addr = NetworkAddress::from_register_address(address);

    let _cost = wallet_client
        .pay_for_storage(std::iter::once(net_addr), true)
        .await?;

    let mut register = client
        .create_register(xor_name, &mut wallet_client, true)
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

#[tokio::test(flavor = "multi_thread")]
async fn storage_payment_register_creation_and_mutation_fails() -> Result<()> {
    let paying_wallet_balance = 55_000_000_005;
    let paying_wallet_dir = TempDir::new()?;

    let (client, paying_wallet) =
        get_client_and_wallet(paying_wallet_dir.path(), paying_wallet_balance).await?;
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);

    let mut rng = rand::thread_rng();
    let xor_name = XorName::random(&mut rng);
    let address = RegisterAddress::new(xor_name, client.signer_pk());
    let net_address =
        NetworkAddress::RegisterAddress(RegisterAddress::new(xor_name, client.signer_pk()));

    let mut no_data_payments = BTreeMap::default();
    no_data_payments.insert(
        net_address.clone(),
        vec![(
            PublicAddress::new(bls::SecretKey::random().public_key()),
            Token::from_nano(0),
        )],
    );

    wallet_client
        .mut_wallet()
        .local_send_storage_payment(no_data_payments, None)?;

    // invalid spends
    client.send(wallet_client.unconfirmed_txs(), true).await?;

    // this should fail to store as the amount paid is not enough
    let mut register = client
        .create_register(xor_name, &mut wallet_client, false)
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
