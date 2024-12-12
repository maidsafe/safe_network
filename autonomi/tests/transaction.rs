// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use ant_logging::LogBuilder;
use ant_protocol::storage::Transaction;
use autonomi::{client::transactions::TransactionError, Client};
use eyre::Result;
use test_utils::{evm::get_funded_wallet, peers_from_env};

#[tokio::test]
async fn transaction_put() -> Result<()> {
    let _log_appender_guard = LogBuilder::init_single_threaded_tokio_test("transaction", false);

    let client = Client::connect(&peers_from_env()?).await?;
    let wallet = get_funded_wallet();

    let key = bls::SecretKey::random();
    let content = [0u8; 32];
    let transaction = Transaction::new(key.public_key(), vec![], content, vec![], &key);

    client.transaction_put(transaction.clone(), &wallet).await?;
    println!("transaction put 1");

    // wait for the transaction to be replicated
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // check that the transaction is stored
    let txs = client.transaction_get(transaction.address()).await?;
    assert_eq!(txs, vec![transaction.clone()]);
    println!("transaction got 1");

    // try put another transaction with the same address
    let content2 = [1u8; 32];
    let transaction2 = Transaction::new(key.public_key(), vec![], content2, vec![], &key);
    let res = client.transaction_put(transaction2.clone(), &wallet).await;

    assert!(matches!(
        res,
        Err(TransactionError::TransactionAlreadyExists(address))
        if address == transaction2.address()
    ));
    Ok(())
}
