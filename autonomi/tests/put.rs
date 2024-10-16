// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#![cfg(feature = "data")]

use alloy::network::TransactionBuilder;
use alloy::providers::Provider;
use autonomi::Client;
use eyre::Result;
use sn_evm::{ProofOfPayment, QuoteHash, TxHash};
use sn_logging::LogBuilder;
use std::collections::HashMap;
use std::time::Duration;
use test_utils::{evm::get_funded_wallet, gen_random_data, peers_from_env};
use tokio::time::sleep;
use xor_name::XorName;

#[tokio::test]
async fn put() -> Result<()> {
    let _log_appender_guard = LogBuilder::init_single_threaded_tokio_test("put", false);

    let client = Client::connect(&peers_from_env()?).await?;
    let wallet = get_funded_wallet();
    let data = gen_random_data(1024 * 1024 * 10);

    let addr = client.data_put(data.clone(), &wallet).await?;

    sleep(Duration::from_secs(10)).await;

    let data_fetched = client.data_get(addr).await?;
    assert_eq!(data, data_fetched, "data fetched should match data put");

    Ok(())
}

// Example of how put would be done using external signers.
#[cfg(feature = "external-signer")]
#[tokio::test]
async fn external_signer_put() -> Result<()> {
    let _log_appender_guard =
        LogBuilder::init_single_threaded_tokio_test("external_signer_put", false);

    let client = Client::connect(&peers_from_env()?).await?;
    let wallet = get_funded_wallet();
    let data = gen_random_data(1024 * 1024 * 10);

    // Encrypt the data as chunks
    let (_data_map_chunk, _chunks, xor_names) =
        autonomi::client::external_signer::encrypt_data(data.clone())?;

    let (quotes, quote_payments, _skipped_chunks) = client
        .get_quotes_for_content_addrs(xor_names.into_iter())
        .await?;

    // Form quotes payment transaction data
    let pay_for_quotes_calldata = autonomi::client::external_signer::pay_for_quotes_calldata(
        wallet.network(),
        quote_payments.into_iter(),
    )?;

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

    let mut payments: HashMap<QuoteHash, TxHash> = HashMap::new();

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
    let proofs: HashMap<XorName, ProofOfPayment> = quotes
        .iter()
        .filter_map(|(xor_name, (_, _, quote))| {
            payments.get(&quote.hash()).map(|tx_hash| {
                (
                    *xor_name,
                    ProofOfPayment {
                        quote: quote.clone(),
                        tx_hash: *tx_hash,
                    },
                )
            })
        })
        .collect();

    let addr = client
        .data_put_with_proof_of_payment(data.clone(), proofs)
        .await?;

    sleep(Duration::from_secs(10)).await;

    let data_fetched = client.data_get(addr).await?;
    assert_eq!(data, data_fetched, "data fetched should match data put");

    Ok(())
}
