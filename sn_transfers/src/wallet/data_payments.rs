// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{MainPubkey, NanoTokens, Transfer};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use xor_name::XorName;

/// The time in seconds that a quote is valid for
pub const QUOTE_EXPIRATION_SECS: u64 = 3600;

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, custom_debug::Debug)]
pub struct Payment {
    /// The transfers we make
    #[debug(skip)]
    pub transfers: Vec<Transfer>,
    /// The Quote we're paying for
    pub quote: PaymentQuote,
}

/// Information relating to a data payment for one address
#[derive(Clone, Serialize, Deserialize)]
pub struct PaymentDetails {
    /// The node we pay
    pub recipient: MainPubkey,
    /// The PeerId (as bytes) of the node we pay.
    /// The PeerId is not stored here to avoid direct dependency with libp2p,
    /// plus it doesn't implement Serialize/Deserialize traits.
    pub peer_id_bytes: Vec<u8>,
    /// The transfer we send to it and its amount as reference
    pub transfer: (Transfer, NanoTokens),
    /// The network Royalties
    pub royalties: (Transfer, NanoTokens),
    /// The original quote
    pub quote: PaymentQuote,
}

impl PaymentDetails {
    /// create a Payment for a PaymentDetails
    pub fn to_payment(&self) -> Payment {
        Payment {
            transfers: vec![self.transfer.0.clone(), self.royalties.0.clone()],
            quote: self.quote.clone(),
        }
    }
}

/// A generic type for signatures
pub type QuoteSignature = Vec<u8>;

/// A payment quote to store data given by a node to a client
/// Note that the PaymentQuote is a contract between the node and itself to make sure the clients aren’t mispaying.
/// It is NOT a contract between the client and the node.
#[derive(
    Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Serialize, Deserialize, custom_debug::Debug,
)]
pub struct PaymentQuote {
    /// the content paid for
    pub content: XorName,
    /// how much the node demands for storing the content
    pub cost: NanoTokens,
    /// the local node time when the quote was created
    pub timestamp: SystemTime,
    /// the node's signature of the 3 fields above
    #[debug(skip)]
    pub signature: QuoteSignature,
}

impl PaymentQuote {
    /// create an empty PaymentQuote
    pub fn zero() -> Self {
        Self {
            content: Default::default(),
            cost: NanoTokens::zero(),
            timestamp: SystemTime::now(),
            signature: vec![],
        }
    }

    /// returns the bytes to be signed
    pub fn bytes_for_signing(xorname: XorName, cost: NanoTokens, timestamp: SystemTime) -> Vec<u8> {
        let mut bytes = xorname.to_vec();
        bytes.extend_from_slice(&cost.to_bytes());
        bytes.extend_from_slice(
            &timestamp
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("Unix epoch to be in the past")
                .as_secs()
                .to_le_bytes(),
        );
        bytes
    }

    /// Returns true) if the quote has not yet expired
    pub fn has_expired(&self) -> bool {
        let now = std::time::SystemTime::now();

        let dur_s = match now.duration_since(self.timestamp) {
            Ok(dur) => dur.as_secs(),
            Err(_) => return true,
        };
        dur_s > QUOTE_EXPIRATION_SECS
    }

    /// test utility to create a dummy quote
    pub fn test_dummy(xorname: XorName, cost: NanoTokens) -> Self {
        Self {
            content: xorname,
            cost,
            timestamp: SystemTime::now(),
            signature: vec![],
        }
    }
}
