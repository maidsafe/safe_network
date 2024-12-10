// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{data::CostError, Client};
use ant_evm::payment_vault::get_market_price;
use ant_evm::{Amount, PaymentQuote, QuotePayment};
use ant_networking::target_arch::{sleep, Duration, Instant};
use ant_networking::{Network, NetworkError};
use ant_protocol::{storage::ChunkAddress, NetworkAddress};
use libp2p::PeerId;
use std::collections::HashMap;
use xor_name::XorName;

// set rate limit to 2 req/s
const TIME_BETWEEN_RPC_CALLS_IN_MS: u64 = 500;

/// A quote for a single address
pub struct QuoteForAddress(pub(crate) Vec<(PeerId, PaymentQuote, Amount)>);

impl QuoteForAddress {
    pub fn price(&self) -> Amount {
        self.0.iter().map(|(_, _, price)| price).sum()
    }
}

/// A quote for many addresses
pub struct StoreQuote(pub(crate) HashMap<XorName, QuoteForAddress>);

impl StoreQuote {
    pub fn price(&self) -> Amount {
        self.0.values().map(|quote| quote.price()).sum()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn payments(&self) -> Vec<QuotePayment> {
        let mut quote_payments = vec![];
        for (_address, quote) in self.0.iter() {
            for (_peer, quote, price) in quote.0.iter() {
                quote_payments.push((quote.hash(), quote.rewards_address, *price));
            }
        }
        quote_payments
    }
}

impl Client {
    pub(crate) async fn get_store_quotes(
        &self,
        content_addrs: impl Iterator<Item = XorName>,
    ) -> Result<StoreQuote, CostError> {
        // get all quotes from nodes
        let futures: Vec<_> = content_addrs
            .into_iter()
            .map(|content_addr| fetch_store_quote_with_retries(&self.network, content_addr))
            .collect();
        let raw_quotes_per_addr = futures::future::try_join_all(futures).await?;

        debug!("Fetched store quotes: {raw_quotes_per_addr:?}");

        // choose the quotes to pay for each address
        let mut quotes_to_pay_per_addr = HashMap::new();
        for (content_addr, raw_quotes) in raw_quotes_per_addr {
            // ask smart contract for the market price
            let mut prices = vec![];

            // rate limit
            let mut maybe_last_call: Option<Instant> = None;

            for (peer, quote) in raw_quotes {
                // NB TODO @mick we need to batch this smart contract call
                // check if we have to wait for the rate limit
                if let Some(last_call) = maybe_last_call {
                    let elapsed = Instant::now() - last_call;
                    let time_to_sleep_ms =
                        TIME_BETWEEN_RPC_CALLS_IN_MS as u128 - elapsed.as_millis();
                    if time_to_sleep_ms > 0 {
                        sleep(Duration::from_millis(time_to_sleep_ms as u64)).await;
                    }
                }

                let price =
                    get_market_price(&self.evm_network, quote.quoting_metrics.clone()).await?;

                maybe_last_call = Some(Instant::now());

                prices.push((peer, quote, price));
            }

            // sort by price
            prices.sort_by(|(_, _, price_a), (_, _, price_b)| price_a.cmp(price_b));

            // we need at least 5 valid quotes to pay for the data
            const MINIMUM_QUOTES_TO_PAY: usize = 5;
            match &prices[..] {
                [first, second, third, fourth, fifth, ..] => {
                    let (p1, q1, _) = first;
                    let (p2, q2, _) = second;

                    // don't pay for the cheapest 2 quotes but include them
                    let first = (*p1, q1.clone(), Amount::ZERO);
                    let second = (*p2, q2.clone(), Amount::ZERO);

                    // pay for the rest
                    quotes_to_pay_per_addr.insert(
                        content_addr,
                        QuoteForAddress(vec![
                            first,
                            second,
                            third.clone(),
                            fourth.clone(),
                            fifth.clone(),
                        ]),
                    );
                }
                _ => {
                    return Err(CostError::NotEnoughNodeQuotes(
                        content_addr,
                        prices.len(),
                        MINIMUM_QUOTES_TO_PAY,
                    ));
                }
            }
        }

        Ok(StoreQuote(quotes_to_pay_per_addr))
    }
}

/// Fetch a store quote for a content address.
async fn fetch_store_quote(
    network: &Network,
    content_addr: XorName,
) -> Result<Vec<(PeerId, PaymentQuote)>, NetworkError> {
    network
        .get_store_quote_from_network(
            NetworkAddress::from_chunk_address(ChunkAddress::new(content_addr)),
            vec![],
        )
        .await
}

/// Fetch a store quote for a content address with a retry strategy.
async fn fetch_store_quote_with_retries(
    network: &Network,
    content_addr: XorName,
) -> Result<(XorName, Vec<(PeerId, PaymentQuote)>), CostError> {
    let mut retries = 0;

    loop {
        match fetch_store_quote(network, content_addr).await {
            Ok(quote) => {
                break Ok((content_addr, quote));
            }
            Err(err) if retries < 2 => {
                retries += 1;
                error!("Error while fetching store quote: {err:?}, retry #{retries}");
            }
            Err(err) => {
                error!(
                    "Error while fetching store quote: {err:?}, stopping after {retries} retries"
                );
                break Err(CostError::CouldNotGetStoreQuote(content_addr));
            }
        }
    }
}
