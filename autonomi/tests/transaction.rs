// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#![cfg(feature = "transactions")]

use ant_logging::LogBuilder;
use ant_protocol::storage::Transaction;
use autonomi::Client;
use eyre::Result;
use test_utils::{evm::get_funded_wallet, peers_from_env};

#[tokio::test]
async fn transaction() -> Result<()> {
    let _log_appender_guard = LogBuilder::init_single_threaded_tokio_test("transaction", false);

    let client = Client::connect(&peers_from_env()?).await?;
    let wallet = get_funded_wallet();

    let key = bls::SecretKey::random();
    let content = [0u8; 32];
    let mut transaction = Transaction::new(key.public_key(), vec![], content, vec![], &key);

    client.transaction_put(transaction, &wallet).await?;

    Ok(())
}
