// Copyright (c) 2023, MaidSafe.
// All rights reserved.
//
// This SAFE Network Software is licensed under the BSD-3-Clause license.
// Please see the LICENSE file for more details.

use super::{NanoTokens, SignedSpend, UniquePubkey};
use crate::Error;
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use std::{cmp::Ordering, collections::BTreeSet};
use tiny_keccak::{Hasher, Sha3};

type Result<T> = std::result::Result<T, Error>;

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize, Hash)]
pub struct InputLedger {
    // TODO: we cannot use UniquePubKey since blsttc doesn't support no-std builds required for Ledger integration
    #[serde(with = "BigArray")]
    pub unique_pubkey: [u8; 48],
    pub amount: NanoTokens,
}

impl InputLedger {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v: Vec<u8> = Default::default();
        v.extend(&self.unique_pubkey);
        v.extend(self.amount.to_bytes());
        v
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize, Hash)]
pub struct Input {
    pub unique_pubkey: UniquePubkey,
    pub amount: NanoTokens,
}

impl Input {
    pub fn new(unique_pubkey: UniquePubkey, amount: u64) -> Self {
        Self {
            unique_pubkey,
            amount: NanoTokens::from(amount),
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v: Vec<u8> = Default::default();
        v.extend(self.unique_pubkey.to_bytes().as_ref());
        v.extend(self.amount.to_bytes());
        v
    }

    pub fn unique_pubkey(&self) -> &UniquePubkey {
        &self.unique_pubkey
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize, Hash)]
pub struct OutputLedger {
    // TODO: we cannot use UniquePubKey since blsttc doesn't support no-std builds required for Ledger integration
    #[serde(with = "BigArray")]
    pub unique_pubkey: [u8; 48],
    pub amount: NanoTokens,
}

impl OutputLedger {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v: Vec<u8> = Default::default();
        v.extend(&self.unique_pubkey);
        v.extend(self.amount.to_bytes());
        v
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct Output {
    pub unique_pubkey: UniquePubkey,
    pub amount: NanoTokens,
}

impl Output {
    pub fn new(unique_pubkey: UniquePubkey, amount: u64) -> Self {
        Self {
            unique_pubkey,
            amount: NanoTokens::from(amount),
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v: Vec<u8> = Default::default();
        v.extend(self.unique_pubkey.to_bytes().as_ref());
        v.extend(self.amount.to_bytes());
        v
    }

    pub fn unique_pubkey(&self) -> &UniquePubkey {
        &self.unique_pubkey
    }
}

#[derive(Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct TransactionLedger {
    pub inputs: [InputLedger; 1],
    pub outputs: [OutputLedger; 1],
}

impl TransactionLedger {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v: Vec<u8> = Default::default();
        for m in self.inputs.iter() {
            v.extend(&m.to_bytes());
        }
        for o in self.outputs.iter() {
            v.extend(&o.to_bytes());
        }
        v
    }
}

#[derive(Clone, Default, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct Transaction {
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>,
}

/// debug method for Transaction which does not print the full content
impl std::fmt::Debug for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // use self.hash to avoid printing the full content
        f.debug_struct("Transaction")
            .field("inputs", &self.inputs.len())
            .field("outputs", &self.outputs.len())
            .field("hash", &self.hash())
            .finish()
    }
}

impl PartialOrd for Transaction {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Transaction {
    fn cmp(&self, other: &Self) -> Ordering {
        self.hash().cmp(&other.hash())
    }
}

impl Transaction {
    pub fn empty() -> Self {
        Self {
            inputs: vec![],
            outputs: vec![],
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v: Vec<u8> = Default::default();
        v.extend("inputs".as_bytes());
        for m in self.inputs.iter() {
            v.extend(&m.to_bytes());
        }
        v.extend("outputs".as_bytes());
        for o in self.outputs.iter() {
            v.extend(&o.to_bytes());
        }
        v.extend("end".as_bytes());
        v
    }

    pub fn hash(&self) -> crate::Hash {
        let mut sha3 = Sha3::v256();
        sha3.update(&self.to_bytes());
        let mut hash = [0; 32];
        sha3.finalize(&mut hash);
        crate::Hash::from(hash)
    }

    /// Quickly check is a transaction is balanced
    fn verify_balanced(&self) -> Result<()> {
        // Check that the input and output tokens are equal.
        let input_sum: u64 = self
            .inputs
            .iter()
            .map(|i| i.amount)
            .try_fold(0, |acc: u64, i| {
                acc.checked_add(i.as_nano()).ok_or(Error::NumericOverflow)
            })?;
        let output_sum: u64 = self
            .outputs
            .iter()
            .map(|o| o.amount)
            .try_fold(0, |acc: u64, o| {
                acc.checked_add(o.as_nano()).ok_or(Error::NumericOverflow)
            })?;

        if input_sum != output_sum {
            Err(Error::UnbalancedTransaction)
        } else {
            Ok(())
        }
    }

    /// Verifies a transaction with Network held signed spends.
    ///
    /// This function assumes that the signed spends where previously fetched from the Network and where not double spent.
    /// This function will verify that:
    /// - the transaction is balanced (sum inputs = sum outputs)
    /// - the inputs and outputs are unique
    /// - the inputs and outputs are different
    /// - the inputs have a corresponding signed spend
    /// - those signed spends are valid and refer to this transaction
    pub fn verify_against_inputs_spent(&self, signed_spends: &BTreeSet<SignedSpend>) -> Result<()> {
        // verify that the tx has at least one input
        if self.inputs.is_empty() {
            return Err(Error::MissingTxInputs);
        }

        // check spends match the inputs
        let input_keys = self
            .inputs
            .iter()
            .map(|i| i.unique_pubkey())
            .collect::<BTreeSet<_>>();
        let signed_spend_keys = signed_spends
            .iter()
            .map(|s| s.unique_pubkey())
            .collect::<BTreeSet<_>>();
        if input_keys != signed_spend_keys {
            debug!("SpendsDoNotMatchInputs: {input_keys:#?} != {signed_spend_keys:#?}");
            return Err(Error::SpendsDoNotMatchInputs);
        }

        // Verify that each output is unique
        let output_pks: BTreeSet<&UniquePubkey> =
            self.outputs.iter().map(|o| (o.unique_pubkey())).collect();
        if output_pks.len() != self.outputs.len() {
            return Err(Error::UniquePubkeyNotUniqueInTx);
        }

        // Verify that each input is unique
        let input_pks: BTreeSet<&UniquePubkey> =
            self.inputs.iter().map(|i| (i.unique_pubkey())).collect();
        if input_pks.len() != self.inputs.len() {
            return Err(Error::UniquePubkeyNotUniqueInTx);
        }

        // Verify that inputs are different from outputs
        if !input_pks.is_disjoint(&output_pks) {
            return Err(Error::UniquePubkeyNotUniqueInTx);
        }

        // Verify that each signed spend is valid
        let spent_tx_hash = self.hash();
        for signed_spend in signed_spends.iter() {
            signed_spend.verify(spent_tx_hash)?;
        }

        // Verify that the transaction is balanced
        self.verify_balanced()
    }
}
