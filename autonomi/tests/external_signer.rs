#![cfg(feature = "external-signer")]

use alloy::network::TransactionBuilder;
use alloy::providers::Provider;
use autonomi::Client;
use sn_evm::{QuoteHash, TxHash};
use sn_logging::LogBuilder;
use std::collections::BTreeMap;
use std::time::Duration;
use test_utils::evm::get_funded_wallet;
use test_utils::{gen_random_data, peers_from_env};
use tokio::time::sleep;

// Example of how put would be done using external signers.
#[tokio::test]
async fn external_signer_put() -> eyre::Result<()> {
    let _log_appender_guard =
        LogBuilder::init_single_threaded_tokio_test("external_signer_put", false);

    let client = Client::connect(&peers_from_env()?).await?;
    let wallet = get_funded_wallet();
    let data = gen_random_data(1024 * 1024 * 10);

    let (quotes, quote_payments, _free_chunks) = client.get_quotes_for_data(data.clone()).await?;

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
    let proofs = autonomi::payment_proof_from_quotes_and_payments(&quotes, &payments);

    let addr = client
        .data_put_with_proof_of_payment(data.clone(), proofs)
        .await?;

    sleep(Duration::from_secs(10)).await;

    let data_fetched = client.data_get(addr).await?;
    assert_eq!(data, data_fetched, "data fetched should match data put");

    Ok(())
}
