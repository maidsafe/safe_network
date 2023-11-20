// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sn_protocol::error::Error as ProtocolError;
use sn_protocol::NetworkAddress;
use sn_transfers::{NanoTokens, PaymentQuote};

use crate::node::Node;

/// The time in seconds that a quote is valid for
/// The time below is 1 hour
/// Short enough for the price to not change too much
/// Long enough for the clients to upload their data with very slow network and in HUGE batches
const QUOTE_EXPIRATION_SECS: u64 = 3600;

impl Node {
    /// Create a payment quote for to reply to a storecost request
    /// This quote contains our signature so we know we can trust its content.
    pub(crate) fn create_quote_for_storecost(
        &self,
        store_cost: Result<NanoTokens, ProtocolError>,
        address: NetworkAddress,
    ) -> Result<PaymentQuote, ProtocolError> {
        let cost = match store_cost {
            Ok(cost) => cost,
            Err(err) => return Err(err),
        };
        let content = address.as_xorname().unwrap_or_default();
        let timestamp = std::time::SystemTime::now();
        let bytes = PaymentQuote::bytes_for_signing(content, cost, timestamp);

        let signature = match self.network.sign(&bytes) {
            Ok(s) => s,
            Err(_) => return Err(ProtocolError::QuoteGenerationFailed),
        };

        let quote = PaymentQuote {
            content,
            cost,
            timestamp,
            signature,
        };

        debug!("Created payment quote for {address:?}: {quote:?}");
        Ok(quote)
    }

    /// Verfiy a payment quote
    /// Make sure the quote is for the address we requested and that it is not expired
    /// Also, verify that we are indeed the ones who created this quote
    /// Reject any quote that does not pass these checks
    pub(crate) fn verify_quote_for_storecost(
        &self,
        quote: PaymentQuote,
        address: &NetworkAddress,
    ) -> Result<(), ProtocolError> {
        debug!("Verifying payment quote for {address:?}: {quote:?}");

        // check address
        if address.as_xorname().unwrap_or_default() != quote.content {
            return Err(ProtocolError::InvalidQuoteContent);
        }

        // check time
        let now = std::time::SystemTime::now();
        let dur_s = match now.duration_since(quote.timestamp) {
            Ok(t) => t.as_secs(),
            Err(_) => return Err(ProtocolError::InvalidQuoteContent),
        };
        if dur_s > QUOTE_EXPIRATION_SECS {
            return Err(ProtocolError::QuoteExpired);
        }

        // check sig
        let bytes = PaymentQuote::bytes_for_signing(quote.content, quote.cost, quote.timestamp);
        let signature = quote.signature;
        if !self.network.verify(&bytes, &signature) {
            return Err(ProtocolError::InvalidQuoteSignature);
        }

        Ok(())
    }
}
