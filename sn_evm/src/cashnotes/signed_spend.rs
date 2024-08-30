// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::spend_reason::SpendReason;
use super::{Hash, NanoTokens, UniquePubkey};
use crate::{
    DerivationIndex, DerivedSecretKey, Result, Signature, SpendAddress, TransferError,
    NETWORK_ROYALTIES_PK,
};

use custom_debug::Debug;
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

/// `SignedSpend`s are the core of the Network's transaction system.
/// They are the data type on the Network used to commit to a transfer of value. Analogous to a transaction in Bitcoin.
/// They are signed piece of data proving the owner's commitment to transfer value.
/// `Spend`s refer to their ancestors and descendants, forming a directed acyclic graph that starts from Genesis.
#[derive(Debug, Clone, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SignedSpend {
    /// The Spend, together with the owner's signature over it, constitutes the SignedSpend.
    pub spend: Spend,
    /// The DerivedSecretKey's signature over the Spend, proving the owner's commitment to the Spend.
    #[debug(skip)]
    pub derived_key_sig: Signature,
}

impl SignedSpend {
    /// Create a new SignedSpend
    pub fn sign(spend: Spend, sk: &DerivedSecretKey) -> Self {
        let derived_key_sig = sk.sign(&spend.to_bytes_for_signing());
        Self {
            spend,
            derived_key_sig,
        }
    }

    /// Get public key of input CashNote.
    pub fn unique_pubkey(&self) -> &UniquePubkey {
        &self.spend.unique_pubkey
    }

    /// Get the SpendAddress where this Spend shoud be
    pub fn address(&self) -> SpendAddress {
        SpendAddress::from_unique_pubkey(&self.spend.unique_pubkey)
    }

    /// Get Nano
    pub fn amount(&self) -> NanoTokens {
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
    /// Checks that:
    /// - it was signed by the DerivedSecretKey that owns the CashNote for this Spend
    /// - the signature is valid
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
    /// - Also handles the case of parent double spends.
    /// - verifies that the parent_spends contains self as an output
    /// - verifies the sum of total inputs equals to the sum of outputs
    pub fn verify_parent_spends(&self, parent_spends: &BTreeSet<SignedSpend>) -> Result<()> {
        let unique_key = self.unique_pubkey();
        trace!("Verifying parent_spends for {self:?}");

        // sort parents by key (identify double spent parents)
        let mut parents_by_key = BTreeMap::new();
        for s in parent_spends {
            parents_by_key
                .entry(s.unique_pubkey())
                .or_insert_with(Vec::new)
                .push(s);
        }

        let mut total_inputs: u64 = 0;
        for (_, spends) in parents_by_key {
            // check for double spend parents
            if spends.len() > 1 {
                error!("While verifying parents of {unique_key}, found a double spend parent: {spends:?}");
                return Err(TransferError::DoubleSpentParent);
            }

            // check that the parent refers to self
            if let Some(parent) = spends.first() {
                match parent.spend.get_output_amount(unique_key) {
                    Some(amount) => {
                        total_inputs += amount.as_nano();
                    }
                    None => {
                        return Err(TransferError::InvalidParentSpend(format!(
                            "Parent spend {:?} doesn't contain self spend {unique_key:?} as one of its output",
                            parent.unique_pubkey()
                        )));
                    }
                }
            }
        }

        let total_outputs = self.amount().as_nano();
        if total_outputs != total_inputs {
            return Err(TransferError::InvalidParentSpend(format!(
                "Parents total input value {total_inputs:?} doesn't match Spend's value {total_outputs:?}"
            )));
        }

        trace!("Validated parent_spends for {unique_key}");
        Ok(())
    }

    /// Create a random Spend for testing
    #[cfg(test)]
    pub(crate) fn random_spend_to(
        rng: &mut rand::prelude::ThreadRng,
        output: UniquePubkey,
        value: u64,
    ) -> Self {
        use crate::MainSecretKey;

        let sk = MainSecretKey::random();
        let index = DerivationIndex::random(rng);
        let derived_sk = sk.derive_key(&index);
        let unique_pubkey = derived_sk.unique_pubkey();
        let reason = SpendReason::default();
        let ancestor = MainSecretKey::random()
            .derive_key(&DerivationIndex::random(rng))
            .unique_pubkey();
        let spend = Spend {
            unique_pubkey,
            reason,
            ancestors: BTreeSet::from_iter(vec![ancestor]),
            descendants: BTreeMap::from_iter(vec![(output, (NanoTokens::from(value)))]),
            royalties: vec![],
        };
        let derived_key_sig = derived_sk.sign(&spend.to_bytes_for_signing());
        Self {
            spend,
            derived_key_sig,
        }
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

/// Represents a spent UniquePubkey on the Network.
/// When a CashNote is spent, a Spend is created with the UniquePubkey of the CashNote.
/// It is then sent to the Network along with the signature of the owner using the DerivedSecretKey matching its UniquePubkey.
/// A Spend can have multiple ancestors (other spends) which will refer to it as a descendant.
/// A Spend's value is equal to the total value given by its ancestors, which one can fetch on the Network to check.
/// A Spend can have multiple descendants (other spends) which will refer to it as an ancestor.
/// A Spend's value is equal to the total value of given to its descendants.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spend {
    /// UniquePubkey of input CashNote that this SignedSpend is proving to be spent.
    pub unique_pubkey: UniquePubkey,
    /// Reason why this CashNote was spent.
    pub reason: SpendReason,
    /// parent spends of this spend
    pub ancestors: BTreeSet<UniquePubkey>,
    /// spends we are parents of along with the amount we commited to give them
    pub descendants: BTreeMap<UniquePubkey, NanoTokens>,
    /// royalties outputs' derivation indexes
    pub royalties: Vec<DerivationIndex>,
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
        bytes.extend(self.reason.hash().as_ref());
        bytes.extend("ancestors".as_bytes());
        for ancestor in self.ancestors.iter() {
            bytes.extend(&ancestor.to_bytes());
        }
        bytes.extend("descendants".as_bytes());
        for (descendant, amount) in self.descendants.iter() {
            bytes.extend(&descendant.to_bytes());
            bytes.extend(amount.to_bytes());
        }
        bytes.extend("royalties".as_bytes());
        for royalty in self.royalties.iter() {
            bytes.extend(royalty.as_bytes());
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
            .map(|amount| amount.as_nano())
            .sum();
        NanoTokens::from(amount)
    }

    /// Returns the royalties descendants of this Spend
    pub fn network_royalties(&self) -> BTreeSet<(UniquePubkey, NanoTokens, DerivationIndex)> {
        let roy_pks: BTreeMap<UniquePubkey, DerivationIndex> = self
            .royalties
            .iter()
            .map(|di| (NETWORK_ROYALTIES_PK.new_unique_pubkey(di), *di))
            .collect();
        self.descendants
            .iter()
            .filter_map(|(pk, amount)| roy_pks.get(pk).map(|di| (*pk, *amount, *di)))
            .collect()
    }

    /// Returns the amount of a particual output target.
    /// None if the target is not one of the outputs
    pub fn get_output_amount(&self, target: &UniquePubkey) -> Option<NanoTokens> {
        self.descendants.get(target).copied()
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
