// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{node::Node, Error, Result};
use sn_networking::Network;
use sn_protocol::{error::Error as ProtocolError, NetworkAddress};
use sn_transfers::{NanoTokens, PaymentQuote};

/// The time in seconds that a quote is valid for
const QUOTE_EXPIRATION_SECS: u64 = 3600;

impl Node {
    pub(crate) fn create_quote_for_storecost(
        network: &Network,
        cost: NanoTokens,
        address: &NetworkAddress,
    ) -> Result<PaymentQuote, ProtocolError> {
        let content = address.as_xorname().unwrap_or_default();
        let timestamp = std::time::SystemTime::now();
        let bytes = PaymentQuote::bytes_for_signing(content, cost, timestamp);

        let Ok(signature) = network.sign(&bytes) else {
            return Err(ProtocolError::QuoteGenerationFailed);
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

    pub(crate) fn verify_quote_for_storecost(
        &self,
        quote: PaymentQuote,
        address: &NetworkAddress,
    ) -> Result<()> {
        debug!("Verifying payment quote for {address:?}: {quote:?}");

        // check address
        if address.as_xorname().unwrap_or_default() != quote.content {
            return Err(Error::InvalidQuoteContent);
        }

        // check time
        let now = std::time::SystemTime::now();
        let dur_s = match now.duration_since(quote.timestamp) {
            Ok(t) => t.as_secs(),
            Err(_) => return Err(Error::InvalidQuoteContent),
        };
        if dur_s > QUOTE_EXPIRATION_SECS {
            return Err(Error::QuoteExpired);
        }

        // check sig
        let bytes = PaymentQuote::bytes_for_signing(quote.content, quote.cost, quote.timestamp);
        let signature = quote.signature;
        if !self.network.verify(&bytes, &signature) {
            return Err(Error::InvalidQuoteSignature);
        }

        Ok(())
    }
}
