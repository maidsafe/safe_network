// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::output_purpose::OutputPurpose;
use super::spend_reason::SpendReason;
use super::{Hash, NanoTokens, UniquePubkey};
use crate::{Result, Signature, SpendAddress, TransferError};

use custom_debug::Debug;
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

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

    /// Get Nano
    pub fn token(&self) -> NanoTokens {
        self.spend.amount()
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
    /// - it was signed by the DerivedSecretKey that owns the CashNote for this Spend
    /// - the signature is valid
    /// - its value didn't change between the two transactions it is involved in (creation and spending)
    ///
    /// It does NOT check:
    /// - if the spend exists on the Network
    /// - the spend's parents and if they exist on the Network
    pub fn verify(&self) -> Result<()> {
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
    /// - verifies that the parent_spends contains self as an output
    /// - verifies the sum of total inputs equals to the sum of outputs
    pub fn verify_parent_spends<'a, T>(&self, parent_spends: T) -> Result<()>
    where
        T: IntoIterator<Item = &'a SignedSpend> + Clone,
    {
        let unique_key = self.unique_pubkey();
        trace!("Verifying parent_spends for {unique_key}");

        let mut total_inputs: u64 = 0;
        for p in parent_spends {
            if let Some(amount) = p.spend.get_output_amount(unique_key) {
                total_inputs += amount.as_nano();
            } else {
                return Err(TransferError::InvalidParentSpend(format!(
                    "Parent spend {:?} doesn't contain self spend {unique_key:?} as one of its output", p.unique_pubkey() 
                )));
            }
        }

        let total_outputs = self.token().as_nano();
        if total_outputs != total_inputs {
            return Err(TransferError::InvalidParentSpend(format!(
                "Parents total_inputs {total_inputs:?} doesn't match total_outputs {total_outputs:?}"
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
/// The claimed `spend.unique_pubkey` must appears in the `ancestor` spends, and the total sum of amount
/// must be equal to the total sum of amount of all outputs (descendants)
#[derive(custom_debug::Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spend {
    /// UniquePubkey of input CashNote that this SignedSpend is proving to be spent.
    pub unique_pubkey: UniquePubkey,
    /// Reason why this CashNote was spent.
    #[debug(skip)]
    pub reason: SpendReason,
    /// Inputs (parent spends) of this spend
    pub ancestors: BTreeSet<UniquePubkey>,
    /// Outputs of this spend
    pub descendants: BTreeMap<UniquePubkey, (NanoTokens, OutputPurpose)>,
}

impl Spend {
    /// Represent this Spend as bytes.
    /// There is no from_bytes, because this function is not symetric as it uses hashes
    pub fn to_bytes_for_signing(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = Default::default();
        bytes.extend(self.unique_pubkey.to_bytes());
        bytes.extend(self.reason.hash().as_ref());
        for ancestor in self.ancestors.iter() {
            bytes.extend(&ancestor.to_bytes());
        }
        for (descendant, (amount, purpose)) in self.descendants.iter() {
            bytes.extend(&descendant.to_bytes());
            bytes.extend(amount.to_bytes());
            bytes.extend(purpose.hash().as_ref());
        }
        bytes
    }

    /// represent this Spend as a Hash
    pub fn hash(&self) -> Hash {
        Hash::hash(&self.to_bytes_for_signing())
    }

    /// Returns the amount to be spent in this Spend
    pub fn amount(&self) -> NanoTokens {
        let amount: u64 = self
            .descendants
            .values()
            .map(|(amount, _)| amount.as_nano())
            .sum();
        NanoTokens::from(amount)
    }

    /// Returns the amount of a particual output target.
    /// None if the target is not one of the outputs
    pub fn get_output_amount(&self, target: &UniquePubkey) -> Option<NanoTokens> {
        if let Some((amount, _)) = self.descendants.get(target) {
            Some(*amount)
        } else {
            None
        }
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
