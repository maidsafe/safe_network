// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{AttoTokens, EvmError};
use evmlib::common::TxHash;
use evmlib::{
    common::{Address as RewardsAddress, QuoteHash},
    utils::dummy_address,
};
use libp2p::{identity::PublicKey, PeerId};
use serde::{Deserialize, Serialize};
#[cfg(not(target_arch = "wasm32"))]
pub use std::time::SystemTime;
#[cfg(target_arch = "wasm32")]
pub use wasmtimer::std::SystemTime;
use xor_name::XorName;

/// The time in seconds that a quote is valid for
pub const QUOTE_EXPIRATION_SECS: u64 = 3600;

/// The margin allowed for live_time
const LIVE_TIME_MARGIN: u64 = 10;

/// The proof of payment for a data payment
#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct ProofOfPayment {
    /// The Quote we're paying for
    pub quote: PaymentQuote,
    /// The transaction hash
    pub tx_hash: TxHash,
}

impl ProofOfPayment {
    pub fn to_peer_id_payee(&self) -> Option<PeerId> {
        let pub_key = PublicKey::try_decode_protobuf(&self.quote.pub_key).ok()?;
        Some(PeerId::from_public_key(&pub_key))
    }
}

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
#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize, custom_debug::Debug)]
pub struct PaymentQuote {
    /// the content paid for
    pub content: XorName,
    /// how much the node demands for storing the content
    pub cost: AttoTokens,
    /// the local node time when the quote was created
    pub timestamp: SystemTime,
    /// quoting metrics being used to generate this quote
    pub quoting_metrics: QuotingMetrics,
    /// list of bad_nodes that client shall not pick as a payee
    /// in `serialised` format to avoid cyclic dependent on sn_protocol
    #[debug(skip)]
    pub bad_nodes: Vec<u8>,
    /// the node's wallet address
    pub rewards_address: RewardsAddress,
    /// the node's libp2p identity public key in bytes (PeerId)
    #[debug(skip)]
    pub pub_key: Vec<u8>,
    /// the node's signature for the quote
    #[debug(skip)]
    pub signature: Vec<u8>,
}

impl PaymentQuote {
    /// create an empty PaymentQuote
    pub fn zero() -> Self {
        Self {
            content: Default::default(),
            cost: AttoTokens::zero(),
            timestamp: SystemTime::now(),
            quoting_metrics: Default::default(),
            bad_nodes: vec![],
            rewards_address: dummy_address(),
            pub_key: vec![],
            signature: vec![],
        }
    }

    pub fn hash(&self) -> QuoteHash {
        let mut bytes = self.bytes_for_sig();
        bytes.extend_from_slice(self.pub_key.as_slice());
        bytes.extend_from_slice(self.signature.as_slice());
        evmlib::cryptography::hash(bytes)
    }

    /// returns the bytes to be signed from the given parameters
    pub fn bytes_for_signing(
        xorname: XorName,
        cost: AttoTokens,
        timestamp: SystemTime,
        quoting_metrics: &QuotingMetrics,
        serialised_bad_nodes: &[u8],
        rewards_address: &RewardsAddress,
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
        let serialised_quoting_metrics = rmp_serde::to_vec(quoting_metrics).unwrap_or_default();
        bytes.extend_from_slice(&serialised_quoting_metrics);
        bytes.extend_from_slice(serialised_bad_nodes);
        bytes.extend_from_slice(rewards_address.as_slice());
        bytes
    }

    /// Returns the bytes to be signed from self
    pub fn bytes_for_sig(&self) -> Vec<u8> {
        Self::bytes_for_signing(
            self.content,
            self.cost,
            self.timestamp,
            &self.quoting_metrics,
            &self.bad_nodes,
            &self.rewards_address,
        )
    }

    /// Returns the peer id of the node that created the quote
    pub fn peer_id(&self) -> Result<PeerId, EvmError> {
        if let Ok(pub_key) = libp2p::identity::PublicKey::try_decode_protobuf(&self.pub_key) {
            Ok(PeerId::from(pub_key.clone()))
        } else {
            error!("Cann't parse PublicKey from protobuf");
            Err(EvmError::InvalidQuotePublicKey)
        }
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

        let bytes = self.bytes_for_sig();

        if !pub_key.verify(&bytes, &self.signature) {
            error!("Signature is not signed by claimed pub_key");
            return false;
        }

        true
    }

    /// Returns true) if the quote has not yet expired
    pub fn has_expired(&self) -> bool {
        let now = SystemTime::now();

        let dur_s = match now.duration_since(self.timestamp) {
            Ok(dur) => dur.as_secs(),
            Err(_) => return true,
        };
        dur_s > QUOTE_EXPIRATION_SECS
    }

    /// test utility to create a dummy quote
    pub fn test_dummy(xorname: XorName, cost: AttoTokens) -> Self {
        Self {
            content: xorname,
            cost,
            timestamp: SystemTime::now(),
            quoting_metrics: Default::default(),
            bad_nodes: vec![],
            pub_key: vec![],
            signature: vec![],
            rewards_address: dummy_address(),
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

        // TODO: Double check if this applies, as this will prevent a node restart with same ID
        if new_quote.quoting_metrics.received_payment_count
            < old_quote.quoting_metrics.received_payment_count
        {
            info!("claimed received_payment_count out of sequence");
            return false;
        }

        let old_elapsed = if let Ok(elapsed) = old_quote.timestamp.elapsed() {
            elapsed
        } else {
            // The elapsed call could fail due to system clock change
            // hence consider the verification succeeded.
            info!("old_quote timestamp elapsed call failure");
            return true;
        };
        let new_elapsed = if let Ok(elapsed) = new_quote.timestamp.elapsed() {
            elapsed
        } else {
            // The elapsed call could fail due to system clock change
            // hence consider the verification succeeded.
            info!("new_quote timestamp elapsed call failure");
            return true;
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
        let bytes = quote.bytes_for_sig();
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
