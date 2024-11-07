#![cfg(feature = "external-signer")]

use alloy::network::TransactionBuilder;
use alloy::providers::Provider;
use autonomi::client::archive::Metadata;
use autonomi::client::archive_private::PrivateArchive;
use autonomi::client::external_signer::encrypt_data;
use autonomi::client::payment::Receipt;
use autonomi::client::vault::user_data::USER_DATA_VAULT_CONTENT_IDENTIFIER;
use autonomi::client::vault::VaultSecretKey;
use autonomi::{receipt_from_quotes_and_payments, Client, Wallet};
use bytes::Bytes;
use sn_evm::{QuoteHash, TxHash};
use sn_logging::LogBuilder;
use std::collections::BTreeMap;
use std::time::Duration;
use test_utils::evm::get_funded_wallet;
use test_utils::{gen_random_data, peers_from_env};
use tokio::time::sleep;
use xor_name::XorName;

async fn pay_for_data(client: &Client, wallet: &Wallet, data: Bytes) -> eyre::Result<Receipt> {
    let (data_map_chunk, chunks) = encrypt_data(data)?;

    let map_xor_name = *data_map_chunk.address().xorname();
    let mut xor_names = vec![map_xor_name];

    for chunk in chunks {
        xor_names.push(*chunk.name());
    }

    pay_for_content_addresses(client, wallet, xor_names.into_iter()).await
}

async fn pay_for_content_addresses(
    client: &Client,
    wallet: &Wallet,
    content_addrs: impl Iterator<Item = XorName>,
) -> eyre::Result<Receipt> {
    let (quotes, quote_payments, _free_chunks) = client
        .get_quotes_for_content_addresses(content_addrs)
        .await?;

    // Form quotes payment transaction data
    let pay_for_quotes_calldata = autonomi::client::external_signer::pay_for_quotes_calldata(
        wallet.network(),
        quote_payments.into_iter(),
    )?;

    // Init an external wallet provider. In the webapp, this would be MetaMask for example
    let provider = wallet.to_provider();

    // Form approve to spend tokens transaction data
    let approve_calldata = autonomi::client::external_signer::approve_to_spend_tokens_calldata(
        wallet.network(),
        pay_for_quotes_calldata.approve_spender,
        pay_for_quotes_calldata.approve_amount,
    );

    // Prepare approve to spend tokens transaction
    let transaction_request = provider
        .transaction_request()
        .with_to(approve_calldata.1)
        .with_input(approve_calldata.0);

    // Send approve to spend tokens transaction
    let _tx_hash = provider
        .send_transaction(transaction_request)
        .await?
        .watch()
        .await?;

    let mut payments: BTreeMap<QuoteHash, TxHash> = Default::default();

    // Execute all quote payment transactions in batches
    for (calldata, quote_hashes) in pay_for_quotes_calldata.batched_calldata_map {
        // Prepare batched quote payments transaction
        let transaction_request = provider
            .transaction_request()
            .with_to(pay_for_quotes_calldata.to)
            .with_input(calldata);

        // Send batched quote payments transaction
        let tx_hash = provider
            .send_transaction(transaction_request)
            .await?
            .watch()
            .await?;

        // Add to payments to be later use to construct the proofs
        for quote_hash in quote_hashes {
            payments.insert(quote_hash, tx_hash);
        }
    }

    // Payment proofs
    Ok(receipt_from_quotes_and_payments(&quotes, &payments))
}

// Example of how put would be done using external signers.
#[tokio::test]
async fn external_signer_put() -> eyre::Result<()> {
    let _log_appender_guard =
        LogBuilder::init_single_threaded_tokio_test("external_signer_put", false);

    let client = Client::connect(&peers_from_env()?).await?;
    let wallet = get_funded_wallet();
    let data = gen_random_data(1024 * 1024 * 10);

    let receipt = pay_for_data(&client, &wallet, data.clone()).await?;

    sleep(Duration::from_secs(5)).await;

    let private_data_access = client
        .private_data_put(data.clone(), receipt.into())
        .await?;

    let mut private_archive = PrivateArchive::new();
    private_archive.add_file(
        "test-file".into(),
        private_data_access,
        Metadata::new_with_size(data.len() as u64),
    );

    let archive_serialized = private_archive.into_bytes()?;

    let receipt = pay_for_data(&client, &wallet, archive_serialized.clone()).await?;

    sleep(Duration::from_secs(5)).await;

    let private_archive_access = client
        .private_archive_put(private_archive, receipt.into())
        .await?;

    let vault_key = VaultSecretKey::random();

    let mut user_data = client
        .get_user_data_from_vault(&vault_key)
        .await
        .unwrap_or_default();

    user_data.add_private_file_archive_with_name(
        private_archive_access.clone(),
        "test-archive".to_string(),
    );

    let (scratch, is_new) = client
        .get_or_create_scratchpad(&vault_key, *USER_DATA_VAULT_CONTENT_IDENTIFIER)
        .await?;

    assert!(is_new, "Scratchpad is not new");

    let scratch_addresses = if is_new {
        scratch.to_xor_name_vec()
    } else {
        vec![]
    };

    let receipt =
        pay_for_content_addresses(&client, &wallet, scratch_addresses.into_iter()).await?;

    sleep(Duration::from_secs(5)).await;

    let _ = client
        .put_user_data_to_vault(&vault_key, receipt.into(), user_data)
        .await?;

    let fetched_user_data = client.get_user_data_from_vault(&vault_key).await?;

    let fetched_private_archive_access = fetched_user_data
        .private_file_archives
        .keys()
        .next()
        .expect("No private archive present in the UserData")
        .clone();

    let fetched_private_archive = client
        .private_archive_get(fetched_private_archive_access)
        .await?;

    let (_, (fetched_private_file_access, _)) = fetched_private_archive
        .map()
        .iter()
        .next()
        .expect("No file present in private archive");

    let fetched_private_file = client
        .private_data_get(fetched_private_file_access.clone())
        .await?;

    assert_eq!(
        fetched_private_file, data,
        "Fetched private data is not identical to the uploaded data"
    );

    Ok(())
}
