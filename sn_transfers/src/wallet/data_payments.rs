// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{MainPubkey, NanoTokens, Transfer};
use libp2p::{identity::PublicKey, PeerId};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use xor_name::XorName;

/// The time in seconds that a quote is valid for
pub const QUOTE_EXPIRATION_SECS: u64 = 3600;

/// The margin allowed for live_time
const LIVE_TIME_MARGIN: u64 = 10;

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

/// Quoting metrics that got used to generate a quote, or to track peer's status.
#[derive(
    Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Serialize, Deserialize, custom_debug::Debug,
)]
pub struct QuotingMetrics {
    /// the records stored
    pub close_records_stored: usize,
    /// the max_records configured
    pub max_records: usize,
    /// number of times that got paid
    pub received_payment_count: usize,
    /// the duration that node keeps connected to the network, measured in hours
    /// TODO: take `restart` into accout
    pub live_time: u64,
}

impl QuotingMetrics {
    /// construct an empty QuotingMetrics
    pub fn new() -> Self {
        Self {
            close_records_stored: 0,
            max_records: 0,
            received_payment_count: 0,
            live_time: 0,
        }
    }
}

impl Default for QuotingMetrics {
    fn default() -> Self {
        Self::new()
    }
}

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
    /// quoting metrics being used to generate this quote
    pub quoting_metrics: QuotingMetrics,
    /// node's public key that can verify the signature
    #[debug(skip)]
    pub pub_key: Vec<u8>,
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
            quoting_metrics: Default::default(),
            pub_key: vec![],
            signature: vec![],
        }
    }

    /// returns the bytes to be signed
    pub fn bytes_for_signing(
        xorname: XorName,
        cost: NanoTokens,
        timestamp: SystemTime,
        quoting_metrics: &QuotingMetrics,
    ) -> Vec<u8> {
        let mut bytes = xorname.to_vec();
        bytes.extend_from_slice(&cost.to_bytes());
        bytes.extend_from_slice(
            &timestamp
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("Unix epoch to be in the past")
                .as_secs()
                .to_le_bytes(),
        );
        let serialised_quoting_metrics = match rmp_serde::to_vec(quoting_metrics) {
            Ok(quoting_metrics_vec) => quoting_metrics_vec,
            Err(_err) => vec![],
        };
        bytes.extend_from_slice(&serialised_quoting_metrics);
        bytes
    }

    /// Check self is signed by the claimed peer
    pub fn check_is_signed_by_claimed_peer(&self, claimed_peer: PeerId) -> bool {
        let pub_key = if let Ok(pub_key) = PublicKey::try_decode_protobuf(&self.pub_key) {
            pub_key
        } else {
            error!("Cann't parse PublicKey from protobuf");
            return false;
        };

        let self_peer_id = PeerId::from(pub_key.clone());

        if self_peer_id != claimed_peer {
            error!("This quote {self:?} of {self_peer_id:?} is not signed by {claimed_peer:?}");
            return false;
        }

        let bytes = Self::bytes_for_signing(
            self.content,
            self.cost,
            self.timestamp,
            &self.quoting_metrics,
        );

        if !pub_key.verify(&bytes, &self.signature) {
            error!("Signature is not signed by claimed pub_key");
            return false;
        }

        true
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
            quoting_metrics: Default::default(),
            pub_key: vec![],
            signature: vec![],
        }
    }

    /// Check whether self is newer than the target quote.
    pub fn is_newer_than(&self, other: &Self) -> bool {
        self.timestamp > other.timestamp
    }

    /// Check against a new quote, verify whether it is a valid one from self perspective.
    /// Returns `true` to flag the `other` quote is valid, from self perspective.
    pub fn historical_verify(&self, other: &Self) -> bool {
        // There is a chance that an old quote got used later than a new quote
        let self_is_newer = self.is_newer_than(other);
        let (old_quote, new_quote) = if self_is_newer {
            (other, self)
        } else {
            (self, other)
        };

        if new_quote.quoting_metrics.live_time < old_quote.quoting_metrics.live_time {
            info!("Claimed live_time out of sequence");
            return false;
        }

        let old_elapsed = if let Ok(elapsed) = old_quote.timestamp.elapsed() {
            elapsed
        } else {
            info!("timestamp failure");
            return false;
        };
        let new_elapsed = if let Ok(elapsed) = new_quote.timestamp.elapsed() {
            elapsed
        } else {
            info!("timestamp failure");
            return false;
        };

        let time_diff = old_elapsed.as_secs().saturating_sub(new_elapsed.as_secs());
        let live_time_diff =
            new_quote.quoting_metrics.live_time - old_quote.quoting_metrics.live_time;
        // In theory, these two shall match, give it a LIVE_TIME_MARGIN to avoid system glitch
        if live_time_diff > time_diff + LIVE_TIME_MARGIN {
            info!("claimed live_time out of sync with the timestamp");
            return false;
        }

        // There could be pruning to be undertaken, also the close range keeps changing as well.
        // Hence `close_records_stored` could be growing or shrinking.
        // Currently not to carry out check on it, just logging to observe the trend.
        debug!(
            "The new quote has {} close records stored, meanwhile old one has {}.",
            new_quote.quoting_metrics.close_records_stored,
            old_quote.quoting_metrics.close_records_stored
        );

        // TODO: Double check if this applies, as this will prevent a node restart with same ID
        if new_quote.quoting_metrics.received_payment_count
            < old_quote.quoting_metrics.received_payment_count
        {
            info!("claimed received_payment_count out of sequence");
            return false;
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use libp2p::identity::Keypair;
    use std::{thread::sleep, time::Duration};

    #[test]
    fn test_is_newer_than() {
        let old_quote = PaymentQuote::zero();
        sleep(Duration::from_millis(100));
        let new_quote = PaymentQuote::zero();
        assert!(new_quote.is_newer_than(&old_quote));
        assert!(!old_quote.is_newer_than(&new_quote));
    }

    #[test]
    fn test_is_signed_by_claimed_peer() {
        let keypair = Keypair::generate_ed25519();
        let peer_id = keypair.public().to_peer_id();

        let false_peer = PeerId::random();

        let mut quote = PaymentQuote::zero();
        let bytes = PaymentQuote::bytes_for_signing(
            quote.content,
            quote.cost,
            quote.timestamp,
            &quote.quoting_metrics,
        );
        let signature = if let Ok(sig) = keypair.sign(&bytes) {
            sig
        } else {
            panic!("Cannot sign the quote!");
        };

        // Check failed with both incorrect pub_key and signature
        assert!(!quote.check_is_signed_by_claimed_peer(peer_id));
        assert!(!quote.check_is_signed_by_claimed_peer(false_peer));

        // Check failed with correct pub_key but incorrect signature
        quote.pub_key = keypair.public().encode_protobuf();
        assert!(!quote.check_is_signed_by_claimed_peer(peer_id));
        assert!(!quote.check_is_signed_by_claimed_peer(false_peer));

        // Check succeed with correct pub_key and signature,
        // and failed with incorrect claimed signer (peer)
        quote.signature = signature;
        assert!(quote.check_is_signed_by_claimed_peer(peer_id));
        assert!(!quote.check_is_signed_by_claimed_peer(false_peer));

        // Check failed with incorrect pub_key but correct signature
        quote.pub_key = Keypair::generate_ed25519().public().encode_protobuf();
        assert!(!quote.check_is_signed_by_claimed_peer(peer_id));
        assert!(!quote.check_is_signed_by_claimed_peer(false_peer));
    }

    #[test]
    fn test_historical_verify() {
        let mut old_quote = PaymentQuote::zero();
        sleep(Duration::from_millis(100));
        let mut new_quote = PaymentQuote::zero();

        // historical_verify will swap quotes to compare based on timeline automatically
        assert!(new_quote.historical_verify(&old_quote));
        assert!(old_quote.historical_verify(&new_quote));

        // Out of sequence received_payment_count shall be detected
        old_quote.quoting_metrics.received_payment_count = 10;
        new_quote.quoting_metrics.received_payment_count = 9;
        assert!(!new_quote.historical_verify(&old_quote));
        assert!(!old_quote.historical_verify(&new_quote));
        // Reset to correct one
        new_quote.quoting_metrics.received_payment_count = 11;
        assert!(new_quote.historical_verify(&old_quote));
        assert!(old_quote.historical_verify(&new_quote));

        // Out of sequence live_time shall be detected
        new_quote.quoting_metrics.live_time = 10;
        old_quote.quoting_metrics.live_time = 11;
        assert!(!new_quote.historical_verify(&old_quote));
        assert!(!old_quote.historical_verify(&new_quote));
        // Out of margin live_time shall be detected
        new_quote.quoting_metrics.live_time = 11 + LIVE_TIME_MARGIN + 1;
        assert!(!new_quote.historical_verify(&old_quote));
        assert!(!old_quote.historical_verify(&new_quote));
        // Reset live_time to be within the margin
        new_quote.quoting_metrics.live_time = 11 + LIVE_TIME_MARGIN - 1;
        assert!(new_quote.historical_verify(&old_quote));
        assert!(old_quote.historical_verify(&new_quote));
    }
}
