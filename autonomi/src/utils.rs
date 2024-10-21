use sn_evm::{ProofOfPayment, QuoteHash, TxHash};
use sn_networking::PayeeQuote;
use std::collections::{BTreeMap, HashMap};
use xor_name::XorName;

pub fn payment_proof_from_quotes_and_payments(
    quotes: &HashMap<XorName, PayeeQuote>,
    payments: &BTreeMap<QuoteHash, TxHash>,
) -> HashMap<XorName, ProofOfPayment> {
    quotes
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
        .collect()
}
