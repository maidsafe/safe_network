// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod error;

pub(crate) use self::error::{Error, Result};

use crate::{
    protocol::{
        address::DbcAddress,
        fees::{FeeCiphers, RequiredFee, SpendPriority, SpendQ},
    },
    storage::SpendStorage,
};

use sn_dbc::{DbcId, DbcTransaction, MainKey, SignedSpend, Token};

use std::collections::{BTreeMap, BTreeSet};
use xor_name::XorName;

const STARTING_FEE: u64 = 4000; // 0.000004 SNT

pub(super) struct Transfers {
    our_name: XorName,
    node_reward_key: MainKey,
    spend_queue: SpendQ<SignedSpend>,
    storage: SpendStorage,
}

impl Transfers {
    /// Create a new instance of `Transfers`.
    pub(crate) fn new(our_name: XorName, node_reward_key: MainKey) -> Self {
        Self {
            our_name,
            node_reward_key,
            spend_queue: SpendQ::with_fee(STARTING_FEE),
            storage: SpendStorage::new(),
        }
    }

    /// Get Spend from local store.
    pub(crate) async fn get(&self, address: DbcAddress) -> Result<SignedSpend> {
        self.storage.get(address).await
    }

    /// Get the required fee for the specified spend priority.
    pub(crate) fn get_required_fee(&self, dbc_id: DbcId, priority: SpendPriority) -> RequiredFee {
        let amount = self.current_fee(priority);

        debug!("Returned amount for priority {priority:?}: {amount}");

        RequiredFee::new(Token::from_nano(amount), dbc_id, &self.node_reward_key)
    }

    /// Get the current fee for the specified spend priority.
    fn current_fee(&self, priority: SpendPriority) -> u64 {
        let spend_q_snapshot = self.spend_queue.snapshot();
        let spend_q_stats = spend_q_snapshot.stats();
        spend_q_stats.map_to_fee(priority)
    }

    /// Tries to add a double spend that was detected by the network.
    pub(crate) async fn try_add_double(
        &mut self,
        a_spend: &SignedSpend,
        b_spend: &SignedSpend,
    ) -> Result<()> {
        self.storage.try_add_double(a_spend, b_spend).await
    }

    /// Tries to add a new spend to the queue.
    ///
    /// All the provided data will be validated, and
    /// if it is valid, the spend will be pushed onto the queue.
    pub(crate) async fn try_add(
        &mut self,
        signed_spend: Box<SignedSpend>,
        source_tx: Box<DbcTransaction>,
        fee_ciphers: BTreeMap<XorName, FeeCiphers>,
        parent_spends: BTreeSet<SignedSpend>,
    ) -> Result<()> {
        // 1. Validate the tx hash.
        // Ensure that the provided src tx is the same as the
        // one we have the hash of in the signed spend.
        let provided_src_tx_hash = source_tx.hash();
        let signed_src_tx_hash = signed_spend.src_tx_hash();

        if provided_src_tx_hash != signed_src_tx_hash {
            return Err(Error::TxSourceMismatch {
                signed_src_tx_hash,
                provided_src_tx_hash,
            });
        }

        // 2. Try extract the fee paid for this spend, and validate it.
        let paid_fee = self.validate_fee(source_tx.as_ref(), fee_ciphers)?;

        // 3. Validate the spend itself.
        self.storage.validate(signed_spend.as_ref()).await?;

        // 4. Validate the parents of the spend.
        // This also ensures that all parent's dst tx's are the same as the src tx of this spend.
        validate_parent_spends(signed_spend.as_ref(), source_tx.as_ref(), parent_spends)?;

        // This spend is valid and goes into the queue.
        self.spend_queue.push(*signed_spend, paid_fee.as_nano());

        // If the rate limit has elapsed..
        if self.spend_queue.elapsed() {
            // .. we process one from the queue.
            // NB: This works for now. We can look at
            // a timeout backstop in coming iterations.
            if let Some((signed_spend, _)) = self.spend_queue.pop() {
                return self.storage.try_add(&signed_spend).await;
            }
        }

        Ok(())
    }

    fn validate_fee(
        &self,
        tx: &DbcTransaction,
        fee_ciphers: BTreeMap<XorName, FeeCiphers>,
    ) -> Result<Token> {
        let fee_paid = decipher_fee(&self.node_reward_key, tx, self.our_name, fee_ciphers)?;

        let spend_q_snapshot = self.spend_queue.snapshot();
        let spend_q_stats = spend_q_snapshot.stats();

        let (valid, lowest) = spend_q_stats.validate_fee(fee_paid.as_nano());

        if !valid {
            return Err(Error::FeeTooLow {
                paid: fee_paid,
                required: Token::from_nano(lowest),
            });
        }

        Ok(fee_paid)
    }
}

/// The src_tx is the tx where the dbc to spend, was created.
/// The signed_spend.dbc_id() shall exist among its outputs.
fn validate_parent_spends(
    signed_spend: &SignedSpend,
    source_tx: &DbcTransaction,
    parent_spends: BTreeSet<SignedSpend>,
) -> Result<()> {
    // The parent_spends will be different spends,
    // one for each input that went into creating the signed_spend.
    for parent_spend in &parent_spends {
        // The dst tx of the parent must be the src tx of the spend.
        if signed_spend.src_tx_hash() != parent_spend.dst_tx_hash() {
            return Err(Error::TxTrailMismatch {
                signed_src_tx_hash: signed_spend.src_tx_hash(),
                parent_dst_tx_hash: parent_spend.dst_tx_hash(),
            });
        }
    }

    // We have gotten all the parent inputs from the network, so the network consider them all valid.
    // But the source tx corresponding to the signed_spend, might not match the parents' details, so that's what we check here.
    let known_parent_blinded_amounts: Vec<_> = parent_spends
        .iter()
        .map(|s| s.spend.blinded_amount)
        .collect();
    // Here we check that the spend that is attempted, was created in a valid tx.
    let src_tx_validity = source_tx.verify(&known_parent_blinded_amounts);
    if src_tx_validity.is_err() {
        return Err(Error::InvalidSourceTxProvided {
            signed_src_tx_hash: signed_spend.src_tx_hash(),
            provided_src_tx_hash: source_tx.hash(),
        });
    }

    // All parents check out.
    Ok(())
}

#[cfg(not(feature = "data-network"))]
fn decipher_fee(
    node_reward_key: &MainKey,
    tx: &DbcTransaction,
    our_name: XorName,
    fee_ciphers: BTreeMap<XorName, FeeCiphers>,
) -> Result<Token> {
    let fee_ciphers = fee_ciphers.get(&our_name).ok_or(Error::MissingFee)?;
    let (dbc_id, revealed_amount) = fee_ciphers.decrypt(node_reward_key)?;
    let output_proof = match tx.outputs.iter().find(|proof| proof.dbc_id() == &dbc_id) {
        Some(proof) => proof,
        None => return Err(Error::MissingFee),
    };

    let blinded_amount = revealed_amount.blinded_amount(&sn_dbc::PedersenGens::default());
    // Since the output proof contains blinded amounts, we can only verify
    // that the amount is what we expect by comparing equality to the blinded
    // amount we build from the decrypted revealed amount (i.e. amount + blinding factor)..
    if blinded_amount != output_proof.blinded_amount() {
        return Err(Error::InvalidFeeBlindedAmount);
    }

    let paid = Token::from_nano(revealed_amount.value());

    // .. and then checking that the revealed amount we have, (that we now
    // know is what the output blinded amount contains, since the above check passed),
    // also is what we expect the amount to be (done in the calling function).
    Ok(paid)
}
