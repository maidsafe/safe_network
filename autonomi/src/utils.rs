use crate::client::payment::Receipt;
use sn_evm::{PaymentQuote, ProofOfPayment, QuoteHash, TxHash};
use sn_networking::PayeeQuote;
use std::collections::{BTreeMap, HashMap};
use xor_name::XorName;

pub fn cost_map_to_quotes(
    cost_map: HashMap<XorName, PayeeQuote>,
) -> HashMap<XorName, PaymentQuote> {
    cost_map.into_iter().map(|(k, (_, _, v))| (k, v)).collect()
}

pub fn receipt_from_cost_map_and_payments(
    cost_map: HashMap<XorName, PayeeQuote>,
    payments: &BTreeMap<QuoteHash, TxHash>,
) -> Receipt {
    let quotes = cost_map_to_quotes(cost_map);
    receipt_from_quotes_and_payments(&quotes, payments)
}

pub fn receipt_from_quotes_and_payments(
    quotes: &HashMap<XorName, PaymentQuote>,
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
