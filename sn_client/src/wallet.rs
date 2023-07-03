// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::Client;

use sn_dbc::{Dbc, PublicAddress, Token};
use sn_protocol::messages::PaymentProof;
use sn_transfers::{
    client_transfers::TransferOutputs,
    payment_proof::build_payment_proofs,
    wallet::{Error, LocalWallet, Result},
};

use futures::future::join_all;
use std::{collections::BTreeMap, iter::Iterator};
use xor_name::XorName;

/// Map from content address name to its corresponding PaymentProof.
pub type PaymentProofsMap = BTreeMap<XorName, PaymentProof>;

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
        trace!("Sending transfer to the network: {transfer:#?}");
        if let Err(error) = self.client.send(transfer.clone()).await {
            warn!("The transfer was not successfully registered in the network: {error:?}. It will be retried later.");
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
    ) -> Result<PaymentProofsMap> {
        // Let's build the payment proofs for list of content addresses
        let (root_hash, audit_trail_info) = build_payment_proofs(content_addrs)?;

        // TODO: request an invoice to network which provides the amount to pay.
        // For now, we just pay 1 nano per Chunk.
        let num_of_addrs = audit_trail_info.len() as u64;
        // We need to just "burn" the amount that corresponds for storage payment.
        let storage_cost = Token::from_nano(num_of_addrs);

        let transfer = self
            .wallet
            .local_send_storage_payment(storage_cost, root_hash, None)
            .await?;

        // send to network
        trace!("Sending transfer to the network: {transfer:#?}");
        if let Err(error) = self.client.send(transfer.clone()).await {
            warn!("The transfer was not successfully registered in the network: {error:?}. It will be retried later.");
            self.unconfirmed_txs.push(transfer);
            return Err(error);
        }

        let spent_ids: Vec<_> = transfer.tx.inputs.iter().map(|i| i.dbc_id()).collect();

        let payment_proofs = audit_trail_info
            .into_iter()
            .map(|(addr, (audit_trail, path))| {
                (
                    addr,
                    PaymentProof {
                        spent_ids: spent_ids.clone(),
                        audit_trail,
                        path,
                    },
                )
            })
            .collect();

        Ok(payment_proofs)
    }

    /// Resend failed txs
    async fn resend_pending_txs(&mut self) {
        for (index, transfer) in self.unconfirmed_txs.clone().into_iter().enumerate() {
            let tx_hash = transfer.tx.hash();
            println!("Trying to republish pending tx: {tx_hash:?}..");
            if self.client.send(transfer.clone()).await.is_ok() {
                println!("Tx {tx_hash:?} was successfully republished!");
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
            trace!("sending spend request to the network: {spend_request:#?}");
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
            "The spends in network were not the same as the ones in the DBC. The parents of this DBC are probably double spends.".into(),
        ))
    }
}

/// Use the client to send a DBC from a local wallet to an address.
/// This marks the spent DBC as spent in the Network
pub async fn send(from: LocalWallet, amount: Token, to: PublicAddress, client: &Client) -> Dbc {
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
