// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::spend_reason::SpendReason;
use super::{Hash, NanoTokens, Transaction, UniquePubkey};
use crate::{DerivationIndex, Result, Signature, SpendAddress, TransferError};

use custom_debug::Debug;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeSet;

/// SignedSpend's are constructed when a CashNote is logged to the spentbook.
#[derive(Debug, Clone, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SignedSpend {
    /// The Spend, which together with signature over it, constitutes the SignedSpend.
    pub spend: Spend,
    /// The DerivedSecretKey's signature over (the hash of) Spend, confirming that the CashNote was intended to be spent.
    #[debug(skip)]
    pub derived_key_sig: Signature,
}

impl SignedSpend {
    /// Get public key of input CashNote.
    pub fn unique_pubkey(&self) -> &UniquePubkey {
        &self.spend.unique_pubkey
    }

    /// Get the SpendAddress where this Spend shoud be
    pub fn address(&self) -> SpendAddress {
        SpendAddress::from_unique_pubkey(&self.spend.unique_pubkey)
    }

    /// Get the hash of the transaction this CashNote is spent in
    pub fn spent_tx_hash(&self) -> Hash {
        self.spend.spent_tx.hash()
    }

    /// Get the transaction this CashNote is spent in
    pub fn spent_tx(&self) -> Transaction {
        self.spend.spent_tx.clone()
    }

    /// Get the hash of the transaction this CashNote was created in
    pub fn parent_tx_hash(&self) -> Hash {
        self.spend.parent_tx.hash()
    }

    /// Get Nano
    pub fn token(&self) -> &NanoTokens {
        &self.spend.amount
    }

    /// Get reason.
    pub fn reason(&self) -> &SpendReason {
        &self.spend.reason
    }

    /// Represent this SignedSpend as bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = Default::default();
        bytes.extend(self.spend.to_bytes_for_signing());
        bytes.extend(self.derived_key_sig.to_bytes());
        bytes
    }

    /// Verify a SignedSpend
    ///
    /// Checks that
    /// - the spend was indeed spent for the given Tx
    /// - it was signed by the DerivedSecretKey that owns the CashNote for this Spend
    /// - the signature is valid
    /// - its value didn't change between the two transactions it is involved in (creation and spending)
    ///
    /// It does NOT check:
    /// - if the spend exists on the Network
    /// - the spend's parents and if they exist on the Network
    pub fn verify(&self, spent_tx_hash: Hash) -> Result<()> {
        // verify that input spent_tx_hash matches self.spent_tx_hash
        if spent_tx_hash != self.spent_tx_hash() {
            return Err(TransferError::TransactionHashMismatch(
                spent_tx_hash,
                self.spent_tx_hash(),
            ));
        }

        // check that the spend is an output of its parent tx
        let parent_tx = &self.spend.parent_tx;
        let unique_key = self.unique_pubkey();
        if !parent_tx
            .outputs
            .iter()
            .any(|o| o.unique_pubkey() == unique_key)
        {
            return Err(TransferError::InvalidParentTx(format!(
                "spend {unique_key} is not an output of the its parent tx: {parent_tx:?}"
            )));
        }

        // check that the spend is an input of its spent tx
        let spent_tx = &self.spend.spent_tx;
        if !spent_tx
            .inputs
            .iter()
            .any(|i| i.unique_pubkey() == unique_key)
        {
            return Err(TransferError::InvalidSpentTx(format!(
                "spend {unique_key} is not an input of the its spent tx: {spent_tx:?}"
            )));
        }

        // check that the value of the spend wasn't tampered with
        let claimed_value = self.spend.amount;
        let creation_value = self
            .spend
            .parent_tx
            .outputs
            .iter()
            .find(|o| o.unique_pubkey == self.spend.unique_pubkey)
            .map(|o| o.amount)
            .unwrap_or(NanoTokens::zero());
        let spent_value = self
            .spend
            .spent_tx
            .inputs
            .iter()
            .find(|i| i.unique_pubkey == self.spend.unique_pubkey)
            .map(|i| i.amount)
            .unwrap_or(NanoTokens::zero());
        if claimed_value != creation_value || creation_value != spent_value {
            return Err(TransferError::InvalidSpendValue(*self.unique_pubkey()));
        }

        // check signature
        // the spend is signed by the DerivedSecretKey
        // corresponding to the UniquePubkey of the CashNote being spent.
        if self
            .spend
            .unique_pubkey
            .verify(&self.derived_key_sig, self.spend.to_bytes_for_signing())
        {
            Ok(())
        } else {
            Err(TransferError::InvalidSpendSignature(*self.unique_pubkey()))
        }
    }

    /// Verify the parents of this Spend, making sure the input parent_spends are ancestors of self.
    /// - Also handles the case of parent double spends.
    /// - verifies that the parent_spends where spent in our spend's parent_tx.
    /// - verifies the parent_tx against the parent_spends
    pub fn verify_parent_spends<'a, T>(&self, parent_spends: T) -> Result<()>
    where
        T: IntoIterator<Item = &'a BTreeSet<SignedSpend>> + Clone,
    {
        let unique_key = self.unique_pubkey();
        trace!("Verifying parent_spends for {unique_key}");

        // Check that the parent where all spent to our parent_tx
        let tx_our_cash_note_was_created_in = self.parent_tx_hash();
        let mut actual_parent_spends = BTreeSet::new();
        for parents in parent_spends.clone().into_iter() {
            if parents.is_empty() {
                error!("No parent spend provided for {unique_key}");
                return Err(TransferError::InvalidParentSpend(
                    "Parent is empty".to_string(),
                ));
            }
            let parent_unique_key = parents
                .iter()
                .map(|p| *p.unique_pubkey())
                .collect::<BTreeSet<_>>();
            if parent_unique_key.len() > 1 {
                error!("While verifying parents of {unique_key}, found a parent double spend, but it contained more than one unique_pubkey. This is invalid. Erroring out.");
                return Err(TransferError::InvalidParentSpend("Invalid parent double spend. More than one unique_pubkey in the parent double spend.".to_string()));
            }

            // if parent is a double spend, get the actual parent among the parent double spends
            let actual_parent = parents
                .iter()
                .find(|p| p.spent_tx_hash() == tx_our_cash_note_was_created_in)
                .cloned();

            match actual_parent {
                Some(actual_parent) => {
                    actual_parent_spends.insert(actual_parent);
                }
                None => {
                    let tx_parent_was_spent_in = parents
                        .iter()
                        .map(|p| p.spent_tx_hash())
                        .collect::<Vec<_>>();
                    return Err(TransferError::InvalidParentSpend(format!(
                        "Parent spend was spent in another transaction. Expected: {tx_our_cash_note_was_created_in:?} Got: {tx_parent_was_spent_in:?}"
                    )));
                }
            }
        }

        // Here we check that the CashNote we're trying to spend was created in a valid tx
        if let Err(e) = self
            .spend
            .parent_tx
            .verify_against_inputs_spent(actual_parent_spends.iter())
        {
            return Err(TransferError::InvalidParentSpend(format!(
                "Parent Tx verification failed: {e:?}"
            )));
        }

        trace!("Validated parent_spends for {unique_key}");
        Ok(())
    }
}

// Impl manually to avoid clippy complaint about Hash conflict.
impl PartialEq for SignedSpend {
    fn eq(&self, other: &Self) -> bool {
        self.spend == other.spend && self.derived_key_sig == other.derived_key_sig
    }
}

impl Eq for SignedSpend {}

impl std::hash::Hash for SignedSpend {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let bytes = self.to_bytes();
        bytes.hash(state);
    }
}

/// Represents the data to be signed by the DerivedSecretKey of the CashNote being spent.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spend {
    /// UniquePubkey of input CashNote that this SignedSpend is proving to be spent.
    pub unique_pubkey: UniquePubkey,
    /// The transaction that the input CashNote is being spent in (where it is an input)
    pub spent_tx: Transaction,
    /// Reason why this CashNote was spent.
    pub reason: SpendReason,
    /// The amount of the input CashNote.
    pub amount: NanoTokens,
    /// The transaction that the input CashNote was created in (where it is an output)
    pub parent_tx: Transaction,
    /// Data to claim the Network Royalties (if any) from the Spend's descendants (outputs in spent_tx)
    pub network_royalties: Vec<DerivationIndex>,
}

impl core::fmt::Debug for Spend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Spend({:?}({:?}))", self.unique_pubkey, self.hash())
    }
}

impl Spend {
    /// Represent this Spend as bytes.
    /// There is no from_bytes, because this function is not symetric as it uses hashes
    pub fn to_bytes_for_signing(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = Default::default();
        bytes.extend(self.unique_pubkey.to_bytes());
        bytes.extend(self.spent_tx.hash().as_ref());
        bytes.extend(self.reason.hash().as_ref());
        bytes.extend(self.amount.to_bytes());
        bytes.extend(self.parent_tx.hash().as_ref());
        bytes
    }

    /// represent this Spend as a Hash
    pub fn hash(&self) -> Hash {
        Hash::hash(&self.to_bytes_for_signing())
    }
}

impl PartialOrd for Spend {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Spend {
    fn cmp(&self, other: &Self) -> Ordering {
        self.unique_pubkey.cmp(&other.unique_pubkey)
    }
}
