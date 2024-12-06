// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use ant_evm::{PaymentQuote, ProofOfPayment, QuoteHash, TxHash};
use ant_evm::{Amount, AttoTokens, QuotePayment};
use ant_networking::{Network, NetworkError, SelectedQuotes};
use ant_protocol::{
    storage::ChunkAddress,
    NetworkAddress,
};
use xor_name::XorName;
use std::collections::{BTreeMap, HashMap};

use crate::client::payment::Receipt;
use super::{data::CostError, Client};

pub struct QuotesToPay {
    pub nodes_to_pay: Vec<QuotePayment>,
    pub nodes_to_upload_to: Vec<SelectedQuotes>,
    pub cost_per_node: AttoTokens,
    pub total_cost: AttoTokens,
}

impl Client {
    pub(crate) async fn get_store_quotes(
        &self,
        content_addrs: impl Iterator<Item = XorName>,
    ) -> Result<HashMap<XorName, QuotesToPay>, CostError> {
        let futures: Vec<_> = content_addrs
            .into_iter()
            .map(|content_addr| fetch_store_quote_with_retries(&self.network, content_addr))
            .collect();

        let quotes = futures::future::try_join_all(futures).await?;

        let mut quotes_to_pay_per_addr = HashMap::new();
        for (content_addr, quotes) in quotes {
            // NB TODO: get cost from smart contract for each quote and set this value to the median of all quotes!
            let cost_per_node = Amount::from(1); 

            // NB TODO: that's all the nodes except the invalid ones (rejected by smart contract)
            let nodes_to_pay: Vec<_> = quotes.iter().map(|(_, q)| (q.hash(), q.rewards_address, cost_per_node)).collect();
            
            // NB TODO: that's the lower half (quotes under or equal to the median price)
            let nodes_to_upload_to = quotes.clone();            

            let total_cost = cost_per_node * Amount::from(nodes_to_pay.len());
            quotes_to_pay_per_addr.insert(content_addr, QuotesToPay {
                nodes_to_pay,
                nodes_to_upload_to,
                cost_per_node: AttoTokens::from_atto(cost_per_node),
                total_cost: AttoTokens::from_atto(total_cost),
            });
        }

        Ok(quotes_to_pay_per_addr)
    }
}

/// Fetch a store quote for a content address.
async fn fetch_store_quote(
    network: &Network,
    content_addr: XorName,
) -> Result<Vec<SelectedQuotes>, NetworkError> {
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
) -> Result<(XorName, Vec<SelectedQuotes>), CostError> {
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

pub fn receipt_from_quotes_and_payments(
    quotes_map: HashMap<XorName, QuotesToPay>,
    payments: &BTreeMap<QuoteHash, TxHash>,
) -> Receipt {
    let quotes = cost_map_to_quotes(quotes_map);
    receipt_from_quotes_and_payments(&quotes, payments)
}

pub fn receipt_from_quotes_and_payments(
    quotes: &HashMap<XorName, QuotesToPay>,
    payments: &BTreeMap<QuoteHash, TxHash>,
) -> Receipt {
    quotes
        .iter()
        .filter_map(|(xor_name, quote)| {
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
        .collect()
}
