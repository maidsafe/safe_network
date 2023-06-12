// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::Client;

use sn_dbc::{Dbc, PublicAddress, Token};
use sn_transfers::{
    client_transfers::TransferOutputs,
    payment_proof::{build_payment_proofs, PaymentProofsMap},
    wallet::{Error, LocalWallet, Result},
};

use bls::SecretKey;
use futures::future::join_all;
use std::iter::Iterator;
use xor_name::XorName;

/// A wallet client can be used to send and
/// receive tokens to/from other wallets.
pub struct WalletClient {
    client: Client,
    wallet: LocalWallet,
    /// These have not yet been successfully confirmed in
    /// the network and need to be republished, to reach network validity.
    /// We maintain the order they were added in, as to republish
    /// them in the correct order, in case any later spend was
    /// dependent on an earlier spend.
    unconfirmed_txs: Vec<TransferOutputs>,
}

impl WalletClient {
    /// Create a new wallet client.
    pub fn new(client: Client, wallet: LocalWallet) -> Self {
        Self {
            client,
            wallet,
            unconfirmed_txs: vec![],
        }
    }

    /// Send tokens to another wallet.
    pub async fn send(&mut self, amount: Token, to: PublicAddress) -> Result<Dbc> {
        // retry previous failures
        self.resend_pending_txs().await;

        // offline transfer
        let transfer = self.wallet.local_send(vec![(amount, to)], None).await?;
        let dbcs = transfer.created_dbcs.clone();

        // send to network
        println!("Sending transfer to the network: {transfer:#?}");
        if let Err(error) = self.client.send(transfer.clone()).await {
            println!("The transfer was not successfully registered in the network: {error:?}. It will be retried later.");
            self.unconfirmed_txs.push(transfer);
        }

        // return created DBCs even if network part failed???
        match &dbcs[..] {
            [info, ..] => Ok(info.dbc.clone()),
            [] => Err(Error::CouldNotSendTokens(
                "No DBCs were returned from the wallet.".into(),
            )),
        }
    }

    /// Send tokens to nodes closest to the data we want to make storage payment for.
    pub async fn pay_for_storage(
        &mut self,
        content_addrs: impl Iterator<Item = &XorName>,
    ) -> Result<(Dbc, PaymentProofsMap)> {
        // TODO: calculate the amount to pay to each node, perhaps just 1 nano to begin with.
        let amount = Token::from_nano(1);

        // FIXME: calculate closest nodes to pay for storage, we are now just burning the output amount.
        let to = vec![(amount, PublicAddress::new(SecretKey::random().public_key()))];

        // Let's build the payment proofs for list of content addresses
        let (reason_hash, payment_proofs) = build_payment_proofs(content_addrs)
            .map_err(|err| Error::StoragePaymentReason(err.to_string()))?;

        let transfer = self.wallet.local_send(to, Some(reason_hash)).await?;

        match &transfer.created_dbcs[..] {
            [info, ..] => Ok((info.dbc.clone(), payment_proofs)),
            [] => Err(Error::CouldNotSendTokens(
                "No DBCs were returned from the wallet.".into(),
            )),
        }
    }

    /// Resend failed txs
    async fn resend_pending_txs(&mut self) {
        for (index, transfer) in self.unconfirmed_txs.clone().into_iter().enumerate() {
            println!("Trying to republish pending tx: {:?}..", transfer.tx_hash);
            if self.client.send(transfer.clone()).await.is_ok() {
                println!("Tx {:?} was successfully republished!", transfer.tx_hash);
                let _ = self.unconfirmed_txs.remove(index);
                // We might want to be _really_ sure and do the below
                // as well, but it's not necessary.
                // use crate::domain::wallet::VerifyingClient;
                // client.verify(tx_hash).await.ok();
            }
        }
    }

    /// Return the wallet.
    pub fn into_wallet(self) -> LocalWallet {
        self.wallet
    }
}

impl Client {
    pub async fn send(&self, transfer: TransferOutputs) -> Result<()> {
        let mut tasks = Vec::new();
        for spend_request in &transfer.all_spend_requests {
            tasks.push(self.network_store_spend(spend_request.clone()));
        }

        for spend_attempt_result in join_all(tasks).await {
            spend_attempt_result.map_err(|err| Error::CouldNotSendTokens(err.to_string()))?;
        }

        Ok(())
    }

    pub async fn verify(&self, dbc: &Dbc) -> Result<()> {
        // We need to get all the spends in the dbc from the network,
        // and compare them to the spends in the dbc, to know if the
        // transfer is considered valid in the network.
        let mut tasks = Vec::new();
        for spend in &dbc.signed_spends {
            tasks.push(self.expect_closest_majority_same(spend.dbc_id()));
        }

        let mut received_spends = std::collections::BTreeSet::new();
        for result in join_all(tasks).await {
            let network_valid_spend =
                result.map_err(|err| Error::CouldNotVerifyTransfer(err.to_string()))?;
            let _ = received_spends.insert(network_valid_spend);
        }

        // If all the spends in the dbc are the same as the ones in the network,
        // we have successfully verified that the dbc is globally recognised and therefor valid.
        if received_spends == dbc.signed_spends {
            return Ok(());
        }
        Err(Error::CouldNotVerifyTransfer(
            "The spends in network were not the same as the ones in the DBC.".into(),
        ))
    }
}

/// Use the client to send a DBC from a local wallet to an address.
/// This marks the spent DBC as spent in the Network
pub async fn send(from: LocalWallet, amount: Token, to: PublicAddress, client: &Client) -> Dbc {
    println!("[DEBUG] send {amount:?} to {to:?}");

    if amount.as_nano() == 0 {
        panic!("Amount must be more than zero.");
    }

    let mut wallet_client = WalletClient::new(client.clone(), from);
    let new_dbc = wallet_client
        .send(amount, to)
        .await
        .expect("Tokens shall be successfully sent.");

    let mut wallet = wallet_client.into_wallet();
    wallet
        .store()
        .await
        .expect("Wallet shall be successfully stored.");
    wallet
        .store_created_dbc(new_dbc.clone())
        .await
        .expect("Created dbc shall be successfully stored.");

    new_dbc
}
