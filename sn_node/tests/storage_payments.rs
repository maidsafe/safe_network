// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod common;

use common::{get_client_and_wallet, init_logging};

use self_encryption::MIN_ENCRYPTABLE_BYTES;
use sn_client::{Client, Error as ClientError, Files, WalletClient};
use sn_dbc::{Hash, Token};
use sn_networking::Error as NetworkError;
use sn_protocol::storage::{Chunk, ChunkAddress};

use assert_fs::TempDir;
use bytes::Bytes;
use eyre::{eyre, Result};
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
        .map(|_| XorName::random(&mut rng))
        .collect::<Vec<_>>();
    println!(
        "Paying for {} random addresses...",
        random_content_addrs.len()
    );

    let _proofs = wallet_client
        .pay_for_storage(random_content_addrs.iter(), true)
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
async fn storage_payment_fails() -> Result<()> {
    init_logging();

    let wallet_dir = TempDir::new()?;
    let (client, mut wallet_client) = get_client_and_wallet(wallet_dir.path(), 15_000).await?;

    // generate a random number (between 50 and 100) of random addresses
    let random_num_of_addrs = rand::thread_rng().gen_range(50..100);
    let storage_cost = Token::from_nano(random_num_of_addrs);

    let mut transfer = wallet_client
        .local_send_storage_payment(storage_cost, Hash::default(), None)
        .await?;
    assert!(transfer.created_dbcs.is_empty());

    // let's corrupt the generated spend in any way
    let mut invalid_signed_spend = transfer.all_spend_requests[0].signed_spend.clone();
    invalid_signed_spend.spend.spent_tx.fee.token = Token::from_nano(random_num_of_addrs + 1);
    transfer.all_spend_requests[0].signed_spend = invalid_signed_spend;

    // Sending now verifies
    let should_err = client.send(transfer.clone(), true).await;

    println!("Verified with fail: {should_err:?}");
    assert!(should_err.is_err());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn storage_payment_proofs_cached_in_wallet() -> Result<()> {
    let wallet_original_balance = 100_000_000_000_000_000;
    let paying_wallet_dir = TempDir::new()?;

    let (client, paying_wallet) =
        get_client_and_wallet(paying_wallet_dir.path(), wallet_original_balance).await?;
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);

    // generate a random number (between 50 and 100) of random addresses
    let mut rng = rand::thread_rng();
    let random_content_addrs = (0..rng.gen_range(50..100))
        .map(|_| XorName::random(&mut rng))
        .collect::<Vec<_>>();

    // let's first pay only for a subset of the addresses
    let subset_len = random_content_addrs.len() / 3;
    println!("Paying for {subset_len} random addresses...",);
    let (proofs, storage_cost) = wallet_client
        .pay_for_storage(random_content_addrs.iter().take(subset_len), true)
        .await?;
    assert_eq!(proofs.len(), subset_len);

    let storage_cost = storage_cost.ok_or(eyre!("Failed to get storage cost"))?;

    // check we've paid only for the subset of addresses, 1 nano per addr
    let new_balance =
        Token::from_nano(wallet_original_balance - (subset_len as u64 * storage_cost.as_nano()));
    println!("Verifying new balance on paying wallet is {new_balance} ...");
    let paying_wallet = wallet_client.into_wallet();
    assert_eq!(paying_wallet.balance(), new_balance);

    // let's verify payment proofs for the subset have been cached in the wallet
    assert!(random_content_addrs
        .iter()
        .take(subset_len)
        .all(|name| paying_wallet.get_payment_proof(name) == proofs.get(name)));

    // now let's request to pay for all addresses, even that we've already paid for a subset of them
    let mut wallet_client = WalletClient::new(client.clone(), paying_wallet);
    let (proofs, storage_cost) = wallet_client
        .pay_for_storage(random_content_addrs.iter(), false)
        .await?;
    assert_eq!(proofs.len(), random_content_addrs.len());

    let storage_cost = storage_cost.ok_or(eyre!("Failed to get storage cost"))?;

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
        .all(|name| paying_wallet.get_payment_proof(name) == proofs.get(name)));

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

    let (proofs, _cost) = wallet_client
        .pay_for_storage(chunks.iter().map(|c| c.name()), true)
        .await?;

    files_api
        .upload_with_proof(content_bytes, &proofs, true)
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

    let (proofs, _cost) = wallet_client
        .pay_for_storage(chunks.iter().map(|c| c.name()), false)
        .await?;

    sleep(Duration::from_secs(5)).await;

    // let's corrupt the proofs, removing spent input ids
    let invalid_proofs: std::collections::BTreeMap<_, _> = proofs
        .clone()
        .into_iter()
        .map(|(a, mut p)| {
            p.spent_ids = vec![];
            (a, p)
        })
        .collect();

    files_api
        .upload_with_proof(content_bytes.clone(), &invalid_proofs, false)
        .await?;
    assert!(matches!(
        files_api.read_bytes(content_addr).await,
        Err(ClientError::Network(NetworkError::RecordNotFound))
    ));

    // let's corrupt the proofs' audit trail
    let invalid_proofs: std::collections::BTreeMap<_, _> = proofs
        .clone()
        .into_iter()
        .map(|(a, mut p)| {
            p.audit_trail = vec![];
            (a, p)
        })
        .collect();

    files_api
        .upload_with_proof(content_bytes.clone(), &invalid_proofs, false)
        .await?;
    assert!(matches!(
        files_api.read_bytes(content_addr).await,
        Err(ClientError::Network(NetworkError::RecordNotFound))
    ));

    // let's corrupt the proofs' audit trail path
    let invalid_proofs: std::collections::BTreeMap<_, _> = proofs
        .clone()
        .into_iter()
        .map(|(a, mut p)| {
            p.path = vec![];
            (a, p)
        })
        .collect();

    files_api
        .upload_with_proof(content_bytes.clone(), &invalid_proofs, false)
        .await?;
    assert!(matches!(
        files_api.read_bytes(content_addr).await,
        Err(ClientError::Network(NetworkError::RecordNotFound))
    ));

    // let's make a payment but only for one chunk/address,
    let (root_hash, _) =
        sn_transfers::payment_proof::build_payment_proofs(chunks.iter().map(|c| c.name()))?;
    let transfer = wallet_client
        .into_wallet()
        .local_send_storage_payment(Token::from_nano(1), root_hash, None)
        .await?;
    client.send(transfer.clone(), false).await?;
    let spent_ids: Vec<_> = transfer.tx.inputs.iter().map(|i| i.dbc_id()).collect();

    sleep(Duration::from_secs(5)).await;

    // and let's link it from the payment proof
    let invalid_proofs: std::collections::BTreeMap<_, _> = proofs
        .clone()
        .into_iter()
        .map(|(a, mut p)| {
            p.spent_ids = spent_ids.clone();
            (a, p)
        })
        .collect();

    // it should fail to store as the amount paid is not enough
    files_api
        .upload_with_proof(content_bytes.clone(), &invalid_proofs, false)
        .await?;
    assert!(matches!(
        files_api.read_bytes(content_addr).await,
        Err(ClientError::Network(NetworkError::RecordNotFound))
    ));

    Ok(())
}
