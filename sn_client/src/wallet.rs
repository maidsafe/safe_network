// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::Error;

use super::{error::Result, Client};
use backoff::{backoff::Backoff, ExponentialBackoff};
use futures::{future::join_all, TryFutureExt};
use libp2p::PeerId;
use sn_networking::target_arch::Instant;
use sn_networking::{GetRecordError, PayeeQuote};
use sn_protocol::NetworkAddress;
use sn_transfers::{
    CashNote, DerivationIndex, HotWallet, MainPubkey, NanoTokens, Payment, PaymentQuote,
    SignedSpend, SpendAddress, Transaction, Transfer, UniquePubkey, WalletError, WalletResult,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    iter::Iterator,
};
use tokio::{
    task::JoinSet,
    time::{sleep, Duration},
};
use xor_name::XorName;

const MAX_RESEND_PENDING_TX_ATTEMPTS: usize = 10;

/// A wallet client can be used to send and receive tokens to and from other wallets.
pub struct WalletClient {
    client: Client,
    wallet: HotWallet,
}

/// The result of the payment made for a set of Content Addresses
pub struct StoragePaymentResult {
    pub storage_cost: NanoTokens,
    pub royalty_fees: NanoTokens,
    pub skipped_chunks: Vec<XorName>,
}

impl WalletClient {
    /// Create a new wallet client.
    ///
    /// # Arguments
    /// * `client` - A instance of the struct [`sn_client::Client`](Client)
    /// * `wallet` - An instance of the struct [`HotWallet`]
    ///
    /// # Example
    /// ```no_run
    /// use sn_client::{Client, WalletClient, Error};
    /// use tempfile::TempDir;
    /// use bls::SecretKey;
    /// use sn_transfers::{HotWallet, MainSecretKey};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// let tmp_path = TempDir::new()?.path().to_owned();
    /// let mut wallet = HotWallet::load_from_path(&tmp_path,Some(MainSecretKey::new(SecretKey::random())))?;
    /// let mut wallet_client = WalletClient::new(client, wallet);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(client: Client, wallet: HotWallet) -> Self {
        Self { client, wallet }
    }

    /// Stores the wallet to the local wallet directory.
    /// # Example
    /// ```no_run
    /// # use sn_client::{Client, WalletClient, Error};
    /// # use tempfile::TempDir;
    /// # use bls::SecretKey;
    /// # use sn_transfers::{HotWallet, MainSecretKey};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// # let tmp_path = TempDir::new()?.path().to_owned();
    /// # let mut wallet = HotWallet::load_from_path(&tmp_path,Some(MainSecretKey::new(SecretKey::random())))?;
    /// let mut wallet_client = WalletClient::new(client, wallet);
    /// wallet_client.store_local_wallet()?;
    /// # Ok(())
    /// # }
    pub fn store_local_wallet(&mut self) -> WalletResult<()> {
        self.wallet.deposit_and_store_to_disk(&vec![])
    }

    /// Display the wallet balance
    /// # Example
    /// ```no_run
    /// // Display the wallet balance in the terminal
    /// # use sn_client::{Client, WalletClient, Error};
    /// # use tempfile::TempDir;
    /// # use bls::SecretKey;
    /// # use sn_transfers::{HotWallet, MainSecretKey};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// # let tmp_path = TempDir::new()?.path().to_owned();
    /// # let mut wallet = HotWallet::load_from_path(&tmp_path,Some(MainSecretKey::new(SecretKey::random())))?;
    /// let mut wallet_client = WalletClient::new(client, wallet);
    /// println!("{}" ,wallet_client.balance());
    /// # Ok(())
    /// # }
    pub fn balance(&self) -> NanoTokens {
        self.wallet.balance()
    }

    /// See if any unconfirmed transactions exist.
    /// # Example
    /// ```no_run
    /// // Print unconfirmed spends to the terminal
    /// # use sn_client::{Client, WalletClient, Error};
    /// # use tempfile::TempDir;
    /// # use bls::SecretKey;
    /// # use sn_transfers::{HotWallet, MainSecretKey};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// # let tmp_path = TempDir::new()?.path().to_owned();
    /// # let mut wallet = HotWallet::load_from_path(&tmp_path,Some(MainSecretKey::new(SecretKey::random())))?;
    /// let mut wallet_client = WalletClient::new(client, wallet);
    /// if wallet_client.unconfirmed_spend_requests_exist() {println!("Unconfirmed spends exist!")};
    /// # Ok(())
    /// # }
    pub fn unconfirmed_spend_requests_exist(&self) -> bool {
        self.wallet.unconfirmed_spend_requests_exist()
    }

    /// Returns the most recent cached Payment for a provided NetworkAddress. This function does not check if the
    /// quote has expired or not. Use get_non_expired_payment_for_addr if you want to get a non expired one.
    ///
    /// If multiple payments have been made to the same address, then we pick the last one as it is the most recent.
    ///
    /// # Arguments
    /// * `address` - The [`NetworkAddress`].
    ///
    /// # Example
    /// ```no_run
    /// // Getting the payment for an address using a random PeerId
    /// # use sn_client::{Client, WalletClient, Error};
    /// # use tempfile::TempDir;
    /// # use bls::SecretKey;
    /// # use sn_transfers::{HotWallet, MainSecretKey};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # use std::io::Bytes;
    /// # let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// # let tmp_path = TempDir::new()?.path().to_owned();
    /// # let mut wallet = HotWallet::load_from_path(&tmp_path,Some(MainSecretKey::new(SecretKey::random())))?;
    /// use libp2p_identity::PeerId;
    /// use sn_protocol::NetworkAddress;
    ///
    /// let mut wallet_client = WalletClient::new(client, wallet);
    /// let network_address = NetworkAddress::from_peer(PeerId::random());
    /// let payment = wallet_client.get_recent_payment_for_addr(&network_address)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_recent_payment_for_addr(
        &self,
        address: &NetworkAddress,
    ) -> WalletResult<(Payment, PeerId)> {
        let xorname = address
            .as_xorname()
            .ok_or(WalletError::InvalidAddressType)?;
        let payment_detail = self.wallet.api().get_recent_payment(&xorname)?;

        let payment = payment_detail.to_payment();
        trace!("Payment retrieved for {xorname:?} from wallet: {payment:?}");
        let peer_id = PeerId::from_bytes(&payment_detail.peer_id_bytes)
            .map_err(|_| WalletError::NoPaymentForAddress(xorname))?;

        Ok((payment, peer_id))
    }

    ///  Returns the all cached Payment for a provided NetworkAddress.
    ///
    /// # Arguments
    /// * `address` - The [`NetworkAddress`].
    ///
    /// # Example
    /// ```no_run
    /// // Getting the payment for an address using a random PeerId
    /// # use sn_client::{Client, WalletClient, Error};
    /// # use tempfile::TempDir;
    /// # use bls::SecretKey;
    /// # use sn_transfers::{HotWallet, MainSecretKey};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # use std::io::Bytes;
    /// # let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// # let tmp_path = TempDir::new()?.path().to_owned();
    /// # let mut wallet = HotWallet::load_from_path(&tmp_path,Some(MainSecretKey::new(SecretKey::random())))?;
    /// use libp2p_identity::PeerId;
    /// use sn_protocol::NetworkAddress;
    ///
    /// let mut wallet_client = WalletClient::new(client, wallet);
    /// let network_address = NetworkAddress::from_peer(PeerId::random());
    /// let payments = wallet_client.get_all_payments_for_addr(&network_address)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_all_payments_for_addr(
        &self,
        address: &NetworkAddress,
    ) -> WalletResult<Vec<(Payment, PeerId)>> {
        let xorname = address
            .as_xorname()
            .ok_or(WalletError::InvalidAddressType)?;
        let payment_details = self.wallet.api().get_all_payments(&xorname)?;

        let payments = payment_details
            .into_iter()
            .map(|details| {
                let payment = details.to_payment();

                match PeerId::from_bytes(&details.peer_id_bytes) {
                    Ok(peer_id) => Ok((payment, peer_id)),
                    Err(_) => Err(WalletError::NoPaymentForAddress(xorname)),
                }
            })
            .collect::<WalletResult<Vec<_>>>()?;

        trace!(
            "{} Payment retrieved for {xorname:?} from wallet: {payments:?}",
            payments.len()
        );

        Ok(payments)
    }

    /// Remove the payment for a given network address from disk.
    ///
    /// # Arguments
    /// * `address` - The [`NetworkAddress`].
    ///
    /// # Example
    /// ```no_run
    /// // Removing a payment address using a random PeerId
    /// # use sn_client::{Client, WalletClient, Error};
    /// # use tempfile::TempDir;
    /// # use bls::SecretKey;
    /// # use sn_transfers::{HotWallet, MainSecretKey};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # use std::io::Bytes;
    /// # let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// # let tmp_path = TempDir::new()?.path().to_owned();
    /// # let mut wallet = HotWallet::load_from_path(&tmp_path,Some(MainSecretKey::new(SecretKey::random())))?;
    /// use libp2p_identity::PeerId;
    /// use sn_protocol::NetworkAddress;
    ///
    /// let mut wallet_client = WalletClient::new(client, wallet);
    /// let network_address = NetworkAddress::from_peer(PeerId::random());
    /// let payment = wallet_client.remove_payment_for_addr(&network_address)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn remove_payment_for_addr(&self, address: &NetworkAddress) -> WalletResult<()> {
        match &address.as_xorname() {
            Some(xorname) => {
                self.wallet.api().remove_payment_transaction(xorname);
                Ok(())
            }
            None => Err(WalletError::InvalidAddressType),
        }
    }

    /// Send tokens to another wallet. Can also verify the store has been successful.
    /// Verification will be attempted via GET request through a Spend on the network.
    ///
    /// # Arguments
    /// * `amount` - [`NanoTokens`].
    /// * `to` - [`MainPubkey`].
    /// * `verify_store` - A boolean to verify store. Set this to true for mandatory verification.
    ///
    /// # Example
    /// ```no_run
    /// # use sn_client::{Client, WalletClient, Error};
    /// # use tempfile::TempDir;
    /// # use bls::SecretKey;
    /// # use sn_transfers::{HotWallet, MainSecretKey};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # use std::io::Bytes;
    /// # let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// # let tmp_path = TempDir::new()?.path().to_owned();
    /// # let mut wallet = HotWallet::load_from_path(&tmp_path,Some(MainSecretKey::new(SecretKey::random())))?;
    /// use sn_transfers::NanoTokens;
    /// let mut wallet_client = WalletClient::new(client, wallet);
    /// let nano = NanoTokens::from(10);
    /// let main_pub_key = MainSecretKey::random().main_pubkey();
    /// let payment = wallet_client.send_cash_note(nano,main_pub_key, true);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send_cash_note(
        &mut self,
        amount: NanoTokens,
        to: MainPubkey,
        verify_store: bool,
    ) -> WalletResult<CashNote> {
        let created_cash_notes = self.wallet.local_send(vec![(amount, to)], None)?;

        // send to network
        if let Err(error) = self
            .client
            .send_spends(
                self.wallet.unconfirmed_spend_requests().iter(),
                verify_store,
            )
            .await
        {
            return Err(WalletError::CouldNotSendMoney(format!(
                "The transfer was not successfully registered in the network: {error:?}"
            )));
        } else {
            // clear unconfirmed txs
            self.wallet.clear_confirmed_spend_requests();
        }

        // return the first CashNote (assuming there is only one because we only sent to one recipient)
        match &created_cash_notes[..] {
            [cashnote] => Ok(cashnote.clone()),
            [_multiple, ..] => Err(WalletError::CouldNotSendMoney(
                "Multiple CashNotes were returned from the transaction when only one was expected. This is a BUG."
                    .into(),
            )),
            [] => Err(WalletError::CouldNotSendMoney(
                "No CashNotes were returned from the wallet.".into(),
            )),
        }
    }

    /// Send signed spends to another wallet.
    /// Can optionally verify if the store has been successful.
    /// Verification will be attempted via GET request through a Spend on the network.
    async fn send_signed_spends(
        &mut self,
        signed_spends: BTreeSet<SignedSpend>,
        tx: Transaction,
        change_id: UniquePubkey,
        output_details: BTreeMap<UniquePubkey, (MainPubkey, DerivationIndex)>,
        verify_store: bool,
    ) -> WalletResult<CashNote> {
        let created_cash_notes =
            self.wallet
                .prepare_signed_transfer(signed_spends, tx, change_id, output_details)?;

        // send to network
        if let Err(error) = self
            .client
            .send_spends(
                self.wallet.unconfirmed_spend_requests().iter(),
                verify_store,
            )
            .await
        {
            return Err(WalletError::CouldNotSendMoney(format!(
                "The transfer was not successfully registered in the network: {error:?}"
            )));
        } else {
            // clear unconfirmed txs
            self.wallet.clear_confirmed_spend_requests();
        }

        // return the first CashNote (assuming there is only one because we only sent to one recipient)
        match &created_cash_notes[..] {
            [cashnote] => Ok(cashnote.clone()),
            [_multiple, ..] => Err(WalletError::CouldNotSendMoney(
                "Multiple CashNotes were returned from the transaction when only one was expected. This is a BUG."
                    .into(),
            )),
            [] => Err(WalletError::CouldNotSendMoney(
                "No CashNotes were returned from the wallet.".into(),
            )),
        }
    }

    /// Get storecost from the network
    /// Returns the MainPubkey of the node to pay and the price in NanoTokens
    ///
    /// # Arguments
    /// - content_addrs - [Iterator]<Items = [`NetworkAddress`]>
    ///
    /// # Returns:
    /// * [WalletResult]<[StoragePaymentResult]>
    ///
    /// # Example
    ///```no_run
    /// # use sn_client::{Client, WalletClient, Error};
    /// # use tempfile::TempDir;
    /// # use bls::SecretKey;
    /// # use sn_transfers::{HotWallet, MainSecretKey};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # use xor_name::XorName;
    /// use sn_protocol::NetworkAddress;
    /// use libp2p_identity::PeerId;
    /// use sn_registers::{Permissions, RegisterAddress};
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// # let tmp_path = TempDir::new()?.path().to_owned();
    /// let mut wallet = HotWallet::load_from_path(&tmp_path,Some(MainSecretKey::new(SecretKey::random())))?;
    /// # let mut rng = rand::thread_rng();
    /// # let xor_name = XorName::random(&mut rng);
    /// let network_address = NetworkAddress::from_peer(PeerId::random());
    /// let mut wallet_client = WalletClient::new(client, wallet);
    /// // Use get_store_cost_at_address(network_address) to get a storecost from the network.
    /// let cost = wallet_client.get_store_cost_at_address(network_address).await?.2.cost.as_nano();
    /// # Ok(())
    /// # }
    pub async fn get_store_cost_at_address(
        &self,
        address: NetworkAddress,
    ) -> WalletResult<PayeeQuote> {
        self.client
            .network
            .get_store_costs_from_network(address, vec![])
            .await
            .map_err(|error| WalletError::CouldNotSendMoney(error.to_string()))
    }

    /// Send tokens to nodes closest to the data we want to make storage payment for. Runs mandatory verification.
    ///
    /// # Arguments
    /// - content_addrs - [Iterator]<Items = [`NetworkAddress`]>
    ///
    /// # Returns:
    /// * [WalletResult]<[StoragePaymentResult]>
    ///
    /// # Example
    ///```no_run
    /// # use sn_client::{Client, WalletClient, Error};
    /// # use tempfile::TempDir;
    /// # use bls::SecretKey;
    /// # use sn_transfers::{HotWallet, MainSecretKey};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # use xor_name::XorName;
    /// use sn_protocol::NetworkAddress;
    /// use sn_registers::{Permissions, RegisterAddress};
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// # let tmp_path = TempDir::new()?.path().to_owned();
    /// # let mut wallet = HotWallet::load_from_path(&tmp_path,Some(MainSecretKey::new(SecretKey::random())))?;
    /// let mut wallet_client = WalletClient::new(client.clone(), wallet);
    /// let mut rng = rand::thread_rng();
    /// let xor_name = XorName::random(&mut rng);
    /// let address = RegisterAddress::new(xor_name, client.signer_pk());
    /// let net_addr = NetworkAddress::from_register_address(address);
    ///
    /// // Paying for a random Register Address
    /// let cost = wallet_client.pay_for_storage(std::iter::once(net_addr)).await?;
    /// # Ok(())
    /// # }
    pub async fn pay_for_storage(
        &mut self,
        content_addrs: impl Iterator<Item = NetworkAddress>,
    ) -> WalletResult<StoragePaymentResult> {
        let verify_store = true;
        let c: Vec<_> = content_addrs.collect();
        // Using default ExponentialBackoff doesn't make sense,
        // as it will just fail after the first payment failure.
        let mut backoff = ExponentialBackoff::default();
        let mut last_err = "No retries".to_string();

        while let Some(delay) = backoff.next_backoff() {
            trace!("Paying for storage (w/backoff retries) for: {:?}", c);
            match self
                .pay_for_storage_once(c.clone().into_iter(), verify_store)
                .await
            {
                Ok(payment_result) => return Ok(payment_result),
                Err(WalletError::CouldNotSendMoney(err)) => {
                    warn!("Attempt to pay for data failed: {err:?}");
                    last_err = err;
                    sleep(delay).await;
                }
                Err(err) => return Err(err),
            }
        }
        Err(WalletError::CouldNotSendMoney(last_err))
    }

    /// Existing chunks will have the store cost set to Zero.
    /// The payment procedure shall be skipped, and the chunk upload as well.
    /// Hence the list of existing chunks will be returned.
    async fn pay_for_storage_once(
        &mut self,
        content_addrs: impl Iterator<Item = NetworkAddress>,
        verify_store: bool,
    ) -> WalletResult<StoragePaymentResult> {
        // get store cost from network in parallel
        let mut tasks = JoinSet::new();
        for content_addr in content_addrs {
            let client = self.client.clone();
            tasks.spawn(async move {
                let cost = client
                    .network
                    .get_store_costs_from_network(content_addr.clone(), vec![])
                    .await
                    .map_err(|error| WalletError::CouldNotSendMoney(error.to_string()));

                debug!("Storecosts retrieved for {content_addr:?} {cost:?}");
                (content_addr, cost)
            });
        }
        debug!("Pending store cost tasks: {:?}", tasks.len());

        // collect store costs
        let mut cost_map = BTreeMap::default();
        let mut skipped_chunks = vec![];
        #[allow(clippy::mutable_key_type)]
        while let Some(res) = tasks.join_next().await {
            match res {
                Ok((content_addr, Ok(cost))) => {
                    if let Some(xorname) = content_addr.as_xorname() {
                        if cost.2.cost == NanoTokens::zero() {
                            skipped_chunks.push(xorname);
                            debug!("Skipped existing chunk {content_addr:?}");
                        } else {
                            debug!("Storecost inserted into payment map for {content_addr:?}");
                            let _ = cost_map.insert(xorname, (cost.1, cost.2, cost.0.to_bytes()));
                        }
                    } else {
                        warn!("Cannot get store cost for a content that is not a data type: {content_addr:?}");
                    }
                }
                Ok((content_addr, Err(err))) => {
                    warn!("Cannot get store cost for {content_addr:?} with error {err:?}");
                    return Err(err);
                }
                Err(e) => {
                    return Err(WalletError::CouldNotSendMoney(format!(
                        "Storecost get task failed: {e:?}"
                    )));
                }
            }
        }
        info!("Storecosts retrieved for all the provided content addrs");

        // pay for records
        let (storage_cost, royalty_fees) = self.pay_for_records(&cost_map, verify_store).await?;
        let res = StoragePaymentResult {
            storage_cost,
            royalty_fees,
            skipped_chunks,
        };
        Ok(res)
    }

    /// Send tokens to nodes closest to the data that we want to make storage payments for.
    /// # Returns:
    ///
    /// * [WalletResult]<([NanoTokens], [NanoTokens])>
    ///
    /// This return contains the amount paid for storage. Including the network royalties fee paid.
    ///
    /// # Params:
    /// * cost_map - [BTreeMap]([XorName],([MainPubkey], [PaymentQuote]))
    /// * verify_store - This optional check can verify if the store has been successful.
    ///
    /// Verification will be attempted via GET request through a Spend on the network.
    ///
    /// # Example
    ///```no_run
    /// # use sn_client::{Client, WalletClient, Error};
    /// # use tempfile::TempDir;
    /// # use bls::SecretKey;
    /// # use sn_transfers::{HotWallet, MainSecretKey};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # use std::collections::BTreeMap;
    /// use xor_name::XorName;
    /// use sn_transfers::{MainPubkey, Payment, PaymentQuote};
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// # let tmp_path = TempDir::new()?.path().to_owned();
    /// # let mut wallet = HotWallet::load_from_path(&tmp_path,Some(MainSecretKey::new(SecretKey::random())))?;
    /// let mut wallet_client = WalletClient::new(client, wallet);
    /// let mut cost_map:BTreeMap<XorName,(MainPubkey,PaymentQuote,Vec<u8>)> = BTreeMap::new();
    /// wallet_client.pay_for_records(&cost_map,true).await?;
    /// # Ok(())
    /// # }
    pub async fn pay_for_records(
        &mut self,
        cost_map: &BTreeMap<XorName, (MainPubkey, PaymentQuote, Vec<u8>)>,
        verify_store: bool,
    ) -> WalletResult<(NanoTokens, NanoTokens)> {
        // Before wallet progress, there shall be no `unconfirmed_spend_requests`
        self.resend_pending_transaction_until_success(verify_store)
            .await?;
        let start = Instant::now();
        let total_cost = self.wallet.local_send_storage_payment(cost_map)?;

        trace!(
            "local_send_storage_payment of {} chunks completed in {:?}",
            cost_map.len(),
            start.elapsed()
        );

        // send to network
        trace!("Sending storage payment transfer to the network");
        let start = Instant::now();
        let spend_attempt_result = self
            .client
            .send_spends(
                self.wallet.unconfirmed_spend_requests().iter(),
                verify_store,
            )
            .await;

        trace!(
            "send_spends of {} chunks completed in {:?}",
            cost_map.len(),
            start.elapsed()
        );

        // Here is bit risky that for the whole bunch of spends to the chunks' store_costs and royalty_fee
        // they will get re-paid again for ALL, if any one of the payment failed to be put.
        let start = Instant::now();
        if let Err(error) = spend_attempt_result {
            warn!("The storage payment transfer was not successfully registered in the network: {error:?}. It will be retried later.");

            // if we have a DoubleSpend error, lets remove the CashNote from the wallet
            if let WalletError::DoubleSpendAttemptedForCashNotes(spent_cash_notes) = &error {
                for cash_note_key in spent_cash_notes {
                    warn!("Removing double spends CashNote from wallet: {cash_note_key:?}");
                    self.wallet.mark_notes_as_spent([cash_note_key]);
                    self.wallet.clear_specific_spend_request(*cash_note_key);
                }
            }

            self.wallet.store_unconfirmed_spend_requests()?;

            return Err(WalletError::CouldNotSendMoney(format!(
                "The storage payment transfer was not successfully registered in the network: {error:?}"
            )));
        } else {
            info!("Spend has completed: {:?}", spend_attempt_result);
            self.wallet.clear_confirmed_spend_requests();
        }
        trace!(
            "clear up spends of {} chunks completed in {:?}",
            cost_map.len(),
            start.elapsed()
        );

        Ok(total_cost)
    }

    /// Resend failed transactions. This can optionally verify the store has been successful.
    /// This will attempt to GET the cash_note from the network.
    async fn resend_pending_transactions(&mut self, verify_store: bool) {
        if self
            .client
            .send_spends(
                self.wallet.unconfirmed_spend_requests().iter(),
                verify_store,
            )
            .await
            .is_ok()
        {
            self.wallet.clear_confirmed_spend_requests();
        }
    }

    /// Try resending failed transactions multiple times until it succeeds or until we reach max attempts.
    async fn resend_pending_transaction_until_success(
        &mut self,
        verify_store: bool,
    ) -> WalletResult<()> {
        let mut did_error = false;
        // Wallet shall be all clear to progress forward.
        let mut attempts = 0;
        while self.wallet.unconfirmed_spend_requests_exist() {
            info!("Pre-Unconfirmed transactions exist, sending again after 1 second...");
            sleep(Duration::from_secs(1)).await;
            self.resend_pending_transactions(verify_store).await;

            if attempts > MAX_RESEND_PENDING_TX_ATTEMPTS {
                // save the error state, but break out of the loop so we can save
                did_error = true;
                break;
            }

            attempts += 1;
        }

        if did_error {
            error!("Wallet has pre-unconfirmed transactions, can't progress further.");
            Err(WalletError::UnconfirmedTxAfterRetries)
        } else {
            Ok(())
        }
    }

    /// Returns the wallet:
    ///
    /// Return type: [HotWallet]
    ///
    /// # Example
    /// ```no_run
    /// # use sn_client::{Client, WalletClient, Error};
    /// # use tempfile::TempDir;
    /// # use bls::SecretKey;
    /// # use sn_transfers::{HotWallet, MainSecretKey};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// # let tmp_path = TempDir::new()?.path().to_owned();
    /// # let mut wallet = HotWallet::load_from_path(&tmp_path,Some(MainSecretKey::new(SecretKey::random())))?;
    /// let mut wallet_client = WalletClient::new(client, wallet);
    /// let paying_wallet = wallet_client.into_wallet();
    /// // Display the wallet balance in the terminal
    /// println!("{}",paying_wallet.balance());
    /// # Ok(())
    /// # }
    pub fn into_wallet(self) -> HotWallet {
        self.wallet
    }

    /// Returns a mutable wallet instance
    ///
    /// Return type: [HotWallet]
    ///
    /// # Example
    /// ```no_run
    /// # use sn_client::{Client, WalletClient, Error};
    /// # use tempfile::TempDir;
    /// # use bls::SecretKey;
    /// # use sn_transfers::{HotWallet, MainSecretKey};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// # let tmp_path = TempDir::new()?.path().to_owned();
    /// # let mut wallet = HotWallet::load_from_path(&tmp_path,Some(MainSecretKey::new(SecretKey::random())))?;
    /// let mut wallet_client = WalletClient::new(client, wallet);
    /// let paying_wallet = wallet_client.mut_wallet();
    /// // Display the mutable wallet balance in the terminal
    /// println!("{}",paying_wallet.balance());
    /// # Ok(())
    /// # }
    pub fn mut_wallet(&mut self) -> &mut HotWallet {
        &mut self.wallet
    }
}

impl Client {
    /// Send spend requests to the network.
    /// This can optionally verify the spends have been correctly stored before returning
    ///
    /// # Arguments
    /// * spend_requests - [Iterator]<[SignedSpend]>
    /// * verify_store - Boolean. Set to true for mandatory verification via a GET request through a Spend on the network.
    ///
    /// # Example
    /// ```no_run
    /// use sn_client::{Client, WalletClient, Error};
    /// # use tempfile::TempDir;
    /// use bls::SecretKey;
    /// use sn_transfers::{HotWallet, MainSecretKey};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// # let tmp_path = TempDir::new()?.path().to_owned();
    /// let mut wallet = HotWallet::load_from_path(&tmp_path,Some(MainSecretKey::new(SecretKey::random())))?;
    /// // An example of sending storage payment transfers over the network with validation
    /// client.send_spends(wallet.unconfirmed_spend_requests().iter(),true).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send_spends(
        &self,
        spend_requests: impl Iterator<Item = &SignedSpend>,
        verify_store: bool,
    ) -> WalletResult<()> {
        let mut tasks = Vec::new();

        // send spends to the network in parralel
        for spend_request in spend_requests {
            debug!(
                "sending spend request to the network: {:?}: {spend_request:#?}",
                spend_request.unique_pubkey()
            );

            let the_task = async move {
                let cash_note_key = spend_request.unique_pubkey();
                let result = self
                    .network_store_spend(spend_request.clone(), verify_store)
                    .await;

                (cash_note_key, result)
            };
            tasks.push(the_task);
        }

        // wait for all the tasks to complete and gather the errors
        let mut errors = Vec::new();
        let mut double_spent_keys = BTreeSet::new();
        for (spend_key, spend_attempt_result) in join_all(tasks).await {
            match spend_attempt_result {
                Err(Error::Network(sn_networking::NetworkError::GetRecordError(
                    GetRecordError::RecordDoesNotMatch(_),
                )))
                | Err(Error::Network(sn_networking::NetworkError::GetRecordError(
                    GetRecordError::SplitRecord { .. },
                ))) => {
                    warn!(
                        "Double spend detected while trying to spend: {:?}",
                        spend_key
                    );
                    double_spent_keys.insert(*spend_key);
                }
                Err(e) => {
                    warn!("Spend request errored out when sent to the network {spend_key:?}: {e}");
                    errors.push((spend_key, e));
                }
                Ok(()) => {
                    trace!("Spend request was successfully sent to the network: {spend_key:?}");
                }
            }
        }

        // report errors accordingly
        // double spend errors in priority as they should be dealt with by the wallet
        if !double_spent_keys.is_empty() {
            return Err(WalletError::DoubleSpendAttemptedForCashNotes(
                double_spent_keys,
            ));
        }
        if !errors.is_empty() {
            let mut err_report = "Failed to send spend requests to the network:".to_string();
            for (spend_key, e) in &errors {
                warn!("Failed to send spend request to the network: {spend_key:?}: {e}");
                err_report.push_str(&format!("{spend_key:?}: {e}"));
            }
            return Err(WalletError::CouldNotSendMoney(err_report));
        }

        Ok(())
    }

    /// Receive a Transfer, verify and redeem CashNotes from the Network.
    ///
    /// # Arguments
    /// * transfer: &[Transfer] - Borrowed value for [Transfer]
    /// * wallet: &[HotWallet] - Borrowed value for [HotWallet]
    ///
    /// # Return Value
    /// * [WalletResult]<[Vec]<[CashNote]>>
    ///
    /// # Example
    /// ```no_run
    /// use sn_client::{Client, WalletClient, Error};
    /// # use tempfile::TempDir;
    /// use bls::SecretKey;
    /// use sn_transfers::{HotWallet, MainSecretKey};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// use tracing::error;
    /// use sn_transfers::Transfer;
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// # let tmp_path = TempDir::new()?.path().to_owned();
    /// let mut wallet = HotWallet::load_from_path(&tmp_path,Some(MainSecretKey::new(SecretKey::random())))?;
    /// let transfer = Transfer::from_hex("13abc").unwrap();
    /// // An example for using client.receive() for cashNotes
    /// let cash_notes = match client.receive(&transfer, &wallet).await {
    ///                 Ok(cash_notes) => cash_notes,
    ///                 Err(err) => {
    ///                     println!("Failed to verify and redeem transfer: {err:?}");
    ///                     error!("Failed to verify and redeem transfer: {err:?}");
    ///                     return Err(err.into());
    ///                 }
    ///             };
    /// # Ok(())
    ///
    /// # }
    /// ```
    pub async fn receive(
        &self,
        transfer: &Transfer,
        wallet: &HotWallet,
    ) -> WalletResult<Vec<CashNote>> {
        let cashnotes = self
            .network
            .verify_and_unpack_transfer(transfer, wallet)
            .map_err(|e| WalletError::CouldNotReceiveMoney(format!("{e:?}")))
            .await?;
        let valuable_cashnotes = self.filter_out_already_spend_cash_notes(cashnotes).await?;
        Ok(valuable_cashnotes)
    }

    /// Check that the redeemed CashNotes are not already spent
    async fn filter_out_already_spend_cash_notes(
        &self,
        mut cash_notes: Vec<CashNote>,
    ) -> WalletResult<Vec<CashNote>> {
        trace!("Validating CashNotes are not already spent");
        let mut tasks = JoinSet::new();
        for cn in &cash_notes {
            let pk = cn.unique_pubkey();
            let addr = SpendAddress::from_unique_pubkey(&pk);
            let self_clone = self.network.clone();
            let _ = tasks.spawn(async move { self_clone.get_spend(addr).await });
        }
        while let Some(result) = tasks.join_next().await {
            let res = result.map_err(|e| WalletError::FailedToGetSpend(format!("{e}")))?;
            match res {
                // if we get a RecordNotFound, it means the CashNote is not spent, which is good
                Err(sn_networking::NetworkError::GetRecordError(
                    GetRecordError::RecordNotFound,
                )) => (),
                // if we get a spend, it means the CashNote is already spent
                Ok(s) => {
                    warn!(
                        "CashNoteRedemption contains a CashNote that is already spent, skipping it: {:?}",
                        s.unique_pubkey()
                    );
                    cash_notes.retain(|c| &c.unique_pubkey() != s.unique_pubkey());
                }
                // report all other errors
                Err(e) => return Err(WalletError::FailedToGetSpend(format!("{e}"))),
            }
        }

        if cash_notes.is_empty() {
            return Err(WalletError::CouldNotVerifyTransfer(
                "All the redeemed CashNotes are already spent".to_string(),
            ));
        }

        Ok(cash_notes)
    }

    /// Verify that the spends referred to (in the CashNote) exist on the network.
    ///
    /// # Arguments
    /// * cash_note - [CashNote]
    ///
    /// # Return value
    /// [WalletResult]
    ///
    /// # Example
    /// ```no_run
    /// use sn_client::{Client, WalletClient, Error};
    /// # use tempfile::TempDir;
    /// use bls::SecretKey;
    /// use sn_transfers::{HotWallet, MainSecretKey};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// use tracing::error;
    /// use sn_transfers::Transfer;
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// # let tmp_path = TempDir::new()?.path().to_owned();
    /// let mut wallet = HotWallet::load_from_path(&tmp_path,Some(MainSecretKey::new(SecretKey::random())))?;
    /// let transfer = Transfer::from_hex("").unwrap();
    /// let cash_notes = client.receive(&transfer, &wallet).await?;
    /// // Verification:
    /// for cash_note in cash_notes {
    ///     println!("{:?}" , client.verify_cashnote(&cash_note).await.unwrap());
    /// }
    /// # Ok(())
    ///
    /// # }
    /// ```
    pub async fn verify_cashnote(&self, cash_note: &CashNote) -> WalletResult<()> {
        // We need to get all the spends in the cash_note from the network,
        // and compare them to the spends in the cash_note, to know if the
        // transfer is considered valid in the network.
        let mut tasks = Vec::new();
        for spend in &cash_note.signed_spends {
            let address = SpendAddress::from_unique_pubkey(spend.unique_pubkey());
            debug!(
                "Getting spend for pubkey {:?} from network at {address:?}",
                spend.unique_pubkey()
            );
            tasks.push(self.get_spend_from_network(address));
        }

        let mut received_spends = std::collections::BTreeSet::new();
        for result in join_all(tasks).await {
            let network_valid_spend =
                result.map_err(|err| WalletError::CouldNotVerifyTransfer(err.to_string()))?;
            let _ = received_spends.insert(network_valid_spend);
        }

        // If all the spends in the cash_note are the same as the ones in the network,
        // we have successfully verified that the cash_note is globally recognised and therefor valid.
        if received_spends == cash_note.signed_spends {
            return Ok(());
        }
        Err(WalletError::CouldNotVerifyTransfer(
            "The spends in network were not the same as the ones in the CashNote. The parents of this CashNote are probably double spends.".into(),
        ))
    }
}

/// Use the client to send a CashNote from a local wallet to an address.
/// This marks the spent CashNote as spent in the Network
///
/// # Arguments
/// * from - [HotWallet]
/// * amount - [NanoTokens]
/// * to - [MainPubkey]
/// * client - [Client]
/// * verify_store - Boolean. Set to true for mandatory verification via a GET request through a Spend on the network.
///
/// # Example
/// ```no_run
/// use sn_client::{Client, WalletClient, Error};
/// # use tempfile::TempDir;
/// use bls::SecretKey;
/// use sn_transfers::{HotWallet, MainSecretKey};
/// # #[tokio::main]
/// # async fn main() -> Result<(),Error>{
/// use tracing::error;
/// use sn_client::send;
/// use sn_transfers::Transfer;
/// let client = Client::new(SecretKey::random(), None, None, None).await?;
/// # let tmp_path = TempDir::new()?.path().to_owned();
/// let mut first_wallet = HotWallet::load_from_path(&tmp_path,Some(MainSecretKey::new(SecretKey::random())))?;
/// let mut second_wallet = HotWallet::load_from_path(&tmp_path,Some(MainSecretKey::new(SecretKey::random())))?;
///     let tokens = send(
///         first_wallet, // From
///         second_wallet.balance(), // To
///         second_wallet.address(), // Amount
///         &client, // Client
///         true, // Verification
///     ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn send(
    from: HotWallet,
    amount: NanoTokens,
    to: MainPubkey,
    client: &Client,
    verify_store: bool,
) -> Result<CashNote> {
    if amount.is_zero() {
        return Err(Error::AmountIsZero);
    }

    let mut wallet_client = WalletClient::new(client.clone(), from);

    if let Err(err) = wallet_client
        .resend_pending_transaction_until_success(verify_store)
        .await
    {
        println!("Wallet has pre-unconfirmed transactions, can't progress further.");
        warn!("Wallet has pre-unconfirmed transactions, can't progress further.");
        return Err(err.into());
    }

    let new_cash_note = wallet_client
        .send_cash_note(amount, to, verify_store)
        .await
        .map_err(|err| {
            error!("Could not send cash note, err: {err:?}");
            err
        })?;

    wallet_client
        .resend_pending_transaction_until_success(verify_store)
        .await?;

    wallet_client
        .into_wallet()
        .deposit_and_store_to_disk(&vec![new_cash_note.clone()])?;

    Ok(new_cash_note)
}

/// Send tokens to another wallet. Can optionally verify the store has been successful.
///
/// Verification will be attempted via GET request through a Spend on the network.
///
/// # Arguments
/// * from - [HotWallet],
/// * client - [Client],
/// * signed_spends - [BTreeSet]<[SignedSpend]>,
/// * transaction - [Transaction],
/// * change_id - [UniquePubkey],
/// * output_details - [BTreeMap]<[UniquePubkey], ([MainPubkey], [DerivationIndex])>,
/// * verify_store - Boolean. Set to true for mandatory verification via a GET request through a Spend on the network.
///
/// # Return value
/// [WalletResult]<[CashNote]>
/// # Example
/// ```no_run
/// use sn_client::{Client, WalletClient, Error};
/// # use tempfile::TempDir;
/// use bls::SecretKey;
/// use sn_transfers::{HotWallet, MainSecretKey};
/// # #[tokio::main]
/// # async fn main() -> Result<(),Error>{
/// use std::collections::{BTreeMap, BTreeSet};
/// use tracing::error;
/// use sn_transfers::{Transaction, Transfer, UniquePubkey};
/// let client = Client::new(SecretKey::random(), None, None, None).await?;
/// # let tmp_path = TempDir::new()?.path().to_owned();
/// let mut wallet = HotWallet::load_from_path(&tmp_path,Some(MainSecretKey::new(SecretKey::random())))?;
/// let transaction = Transaction {inputs: Vec::new(),outputs: Vec::new(),};
/// let secret_key = UniquePubkey::new(SecretKey::random().public_key());
///
/// println!("Broadcasting the transaction to the network...");
///  let cash_note = sn_client::broadcast_signed_spends(
///     wallet,
///     &client,
///     BTreeSet::default(),
///     transaction,
///     secret_key,
///     BTreeMap::new(),
///     true
///  ).await?;
///
/// # Ok(())
/// # }
/// ```
pub async fn broadcast_signed_spends(
    from: HotWallet,
    client: &Client,
    signed_spends: BTreeSet<SignedSpend>,
    tx: Transaction,
    change_id: UniquePubkey,
    output_details: BTreeMap<UniquePubkey, (MainPubkey, DerivationIndex)>,
    verify_store: bool,
) -> WalletResult<CashNote> {
    let mut wallet_client = WalletClient::new(client.clone(), from);

    // Wallet shall be all clear to progress forward.
    if let Err(err) = wallet_client
        .resend_pending_transaction_until_success(verify_store)
        .await
    {
        println!("Wallet has pre-unconfirmed transactions, can't progress further.");
        return Err(err);
    }

    let new_cash_note = wallet_client
        .send_signed_spends(signed_spends, tx, change_id, output_details, verify_store)
        .await
        .map_err(|err| {
            error!("Could not send signed spends, err: {err:?}");
            err
        })?;

    wallet_client
        .resend_pending_transaction_until_success(verify_store)
        .await?;

    wallet_client
        .into_wallet()
        .deposit_and_store_to_disk(&vec![new_cash_note.clone()])?;

    Ok(new_cash_note)
}
