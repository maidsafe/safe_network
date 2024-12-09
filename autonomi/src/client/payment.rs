use crate::client::data::PayError;
use crate::client::quote::StoreQuote;
use crate::Client;
use ant_evm::{AttoTokens, EncodedPeerId, EvmWallet, ProofOfPayment, QuoteHash, TxHash};
use std::collections::{BTreeMap, HashMap};
use xor_name::XorName;

/// Contains the proof of payments for each XOR address and the amount paid
pub type Receipt = HashMap<XorName, (ProofOfPayment, AttoTokens)>;

pub fn receipt_from_store_quotes_and_payments(
    quotes: StoreQuote,
    payments: BTreeMap<QuoteHash, TxHash>,
) -> Receipt {
    let mut receipt = Receipt::new();

    for (content_addr, quote_for_address) in quotes.0 {
        let price = AttoTokens::from_atto(quote_for_address.price());

        let mut proof_of_payment = ProofOfPayment {
            peer_quotes: vec![],
        };

        for (peer_id, quote, _amount) in quote_for_address.0 {
            // skip quotes that haven't been paid
            if !payments.contains_key(&quote.hash()) {
                continue;
            }

            proof_of_payment
                .peer_quotes
                .push((EncodedPeerId::from(peer_id), quote));
        }

        // skip empty proofs
        if proof_of_payment.peer_quotes.is_empty() {
            continue;
        }

        receipt.insert(content_addr, (proof_of_payment, price));
    }

    receipt
}

/// Payment options for data payments.
#[derive(Clone)]
pub enum PaymentOption {
    Wallet(EvmWallet),
    Receipt(Receipt),
}

impl From<EvmWallet> for PaymentOption {
    fn from(value: EvmWallet) -> Self {
        PaymentOption::Wallet(value)
    }
}

impl From<&EvmWallet> for PaymentOption {
    fn from(value: &EvmWallet) -> Self {
        PaymentOption::Wallet(value.clone())
    }
}

impl From<Receipt> for PaymentOption {
    fn from(value: Receipt) -> Self {
        PaymentOption::Receipt(value)
    }
}

impl Client {
    pub(crate) async fn pay_for_content_addrs(
        &self,
        content_addrs: impl Iterator<Item = XorName>,
        payment_option: PaymentOption,
    ) -> Result<Receipt, PayError> {
        match payment_option {
            PaymentOption::Wallet(wallet) => {
                let receipt = self.pay(content_addrs, &wallet).await?;
                Ok(receipt)
            }
            PaymentOption::Receipt(receipt) => Ok(receipt),
        }
    }
}
