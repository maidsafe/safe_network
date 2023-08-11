// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod common;

use common::{get_client_and_wallet, get_wallet, init_logging};

use sn_client::send;

use sn_dbc::{random_derivation_index, rng, Token};
use sn_transfers::client_transfers::create_transfer;

use assert_fs::TempDir;
use eyre::Result;
use tokio::time::sleep;

#[tokio::test(flavor = "multi_thread")]
async fn dbc_transfer_multiple_sequential_succeed() -> Result<()> {
    init_logging();

    let first_wallet_balance = 1_000_000_000;
    let first_wallet_dir = TempDir::new()?;

    let (client, first_wallet) =
        get_client_and_wallet(first_wallet_dir.path(), first_wallet_balance).await?;

    let second_wallet_balance = Token::from_nano(first_wallet_balance / 2);
    println!("Transferring from first wallet to second wallet: {second_wallet_balance}.");
    let second_wallet_dir = TempDir::new()?;
    let mut second_wallet = get_wallet(second_wallet_dir.path()).await;

    assert_eq!(second_wallet.balance(), Token::zero());

    let tokens = send(
        first_wallet,
        second_wallet_balance,
        second_wallet.address(),
        &client,
        true,
    )
    .await?;
    println!("Verifying the transfer from first wallet...");
    client.verify(&tokens).await?;
    second_wallet.deposit(vec![tokens]);
    assert_eq!(second_wallet.balance(), second_wallet_balance);
    println!("Tokens deposited to second wallet: {second_wallet_balance}.");

    let first_wallet = get_wallet(&first_wallet_dir).await;
    assert!(second_wallet_balance.as_nano() == first_wallet.balance().as_nano());

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn dbc_transfer_double_spend_fail() -> Result<()> {
    init_logging();

    // create 1 wallet add money from faucet
    let first_wallet_balance = 1_000_000_000;
    let first_wallet_dir = TempDir::new()?;

    let (client, first_wallet) =
        get_client_and_wallet(first_wallet_dir.path(), first_wallet_balance).await?;

    // create wallet 2 and 3 to receive money from 1
    let second_wallet_dir = TempDir::new()?;
    let second_wallet = get_wallet(second_wallet_dir.path()).await;
    assert_eq!(second_wallet.balance(), Token::zero());
    let third_wallet_dir = TempDir::new()?;
    let third_wallet = get_wallet(third_wallet_dir.path()).await;
    assert_eq!(third_wallet.balance(), Token::zero());

    // manually forge two transfers of the same source
    let amount = Token::from_nano(first_wallet_balance / 3);
    let to1 = first_wallet.address();
    let to2 = second_wallet.address();
    let to3 = third_wallet.address();

    let some_dbcs = first_wallet.available_dbcs();
    let same_dbcs = some_dbcs.clone();

    let mut rng = rng::thread_rng();

    let to2_unique_key = (amount, to2, random_derivation_index(&mut rng));
    let to3_unique_key = (amount, to3, random_derivation_index(&mut rng));
    let reason_hash: sn_dbc::Hash = None.unwrap_or_default();

    let transfer_to_2 = create_transfer(some_dbcs, vec![to2_unique_key], to1, reason_hash).unwrap();
    let transfer_to_3 = create_transfer(same_dbcs, vec![to3_unique_key], to1, reason_hash).unwrap();

    // send both transfers to the network
    // upload won't error out, only error out during verification.
    println!("Sending both transfers to the network...");
    let res = client.send_without_verify(transfer_to_2.clone()).await;
    assert!(res.is_ok());
    let res = client.send_without_verify(transfer_to_3.clone()).await;
    assert!(res.is_ok());

    // check the DBCs, it should fail
    println!("Verifying the transfers from first wallet...");

    let dbcs_for_2: Vec<_> = transfer_to_2.created_dbcs.clone();
    let dbcs_for_3: Vec<_> = transfer_to_3.created_dbcs.clone();

    let mut could_err1 = client.verify(&dbcs_for_2[0]).await;
    let mut could_err2 = client.verify(&dbcs_for_3[0]).await;
    println!("Verifying at least one fails and one is ok: {could_err1:?} {could_err2:?}");
    let mut dbcs_2_ok_3_fail = false;
    let mut dbcs_2_fail_3_ok = false;

    let mut one_transfer_succeeded = false;
    while !one_transfer_succeeded {
        could_err1 = client.verify(&transfer_to_2.created_dbcs[0]).await;
        could_err2 = client.verify(&transfer_to_3.created_dbcs[0]).await;

        dbcs_2_ok_3_fail = could_err1.is_ok() && could_err2.is_err();
        dbcs_2_fail_3_ok = could_err1.is_err() && could_err2.is_ok();
        one_transfer_succeeded = dbcs_2_ok_3_fail || dbcs_2_fail_3_ok;

        // small wait before we try again
        sleep(tokio::time::Duration::from_secs(1)).await;
    }

    assert!(
        dbcs_2_ok_3_fail || dbcs_2_fail_3_ok,
        "one transfer should be valid and the other have failed"
    );

    Ok(())
}
