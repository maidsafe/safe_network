// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use std::cmp::min;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;

use crate::UniquePubkey;
use crate::{
    error::Result, CashNote, DerivationIndex, MainPubkey, MainSecretKey, NanoTokens, OutputPurpose,
    SignedSpend, SignedTransaction, Spend, SpendReason, TransferError,
};

use serde::{Deserialize, Serialize};

/// A local transaction that has not been signed yet
/// All fields are private to prevent bad useage
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct UnsignedTransaction {
    /// Output CashNotes stripped of their parent spends, unuseable as is
    output_cashnotes_without_spends: Vec<CashNote>,
    /// Change CashNote stripped of its parent spends, unuseable as is
    change_cashnote_without_spends: Option<CashNote>,
    /// Spends waiting to be signed along with their secret derivation index
    spends: Vec<(Spend, DerivationIndex)>,
}

impl Debug for UnsignedTransaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnsignedTransaction")
            .field(
                "spends",
                &self.spends.iter().map(|(s, _)| s).collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl UnsignedTransaction {
    /// Create a new `UnsignedTransaction` with the given inputs and outputs
    /// This function will perform a distribution of the input value to the outputs
    /// In the figure below, inputs and outputs represent `CashNote`s,
    /// which are spent thus creating spends that commit to a transfer of value to the outputs.
    /// The value of the outputs is the sum of the values given to them by the inputs.
    ///
    /// ```text
    ///
    ///           inputA(7)     inputB(5)
    ///               |            |
    ///               |            |
    ///             spend1        spend2
    ///             /    \        /  \  \__________
    ///            5      2      2    1             2
    ///           /        \    /      \             \
    ///  outputA(5)      outputB(4)   outputC(1)    change(2)
    ///
    /// ```
    ///
    /// Once created, the `UnsignedTransaction` can be signed with the owner's `MainSecretKey` using the `sign` method
    pub fn new(
        available_cash_notes: Vec<CashNote>,
        recipients: Vec<(NanoTokens, MainPubkey, DerivationIndex, OutputPurpose)>,
        change_to: MainPubkey,
        input_reason_hash: SpendReason,
    ) -> Result<Self> {
        // check total output amount
        let total_output_amount = recipients
            .iter()
            .try_fold(NanoTokens::zero(), |total, (amount, _, _, _)| {
                total.checked_add(*amount)
            })
            .ok_or(TransferError::ExcessiveNanoValue)?;

        // pick input cash notes
        let (chosen_input_cn, _theory_change_amount) =
            select_inputs(available_cash_notes, total_output_amount)?;

        // create empty output cash notes for recipients
        let outputs: Vec<(CashNote, NanoTokens, OutputPurpose)> = recipients
            .iter()
            .map(|(amount, main_pk, derivation_index, purpose)| {
                let cn = CashNote {
                    parent_spends: BTreeSet::new(),
                    main_pubkey: *main_pk,
                    derivation_index: *derivation_index,
                };
                (cn, *amount, purpose.clone())
            })
            .collect();

        // distribute value from inputs to output cash notes
        let mut spends = Vec::new();
        let mut change_cn = None;
        let mut outputs_iter = outputs.iter();
        for input in chosen_input_cn {
            let input_key = input.unique_pubkey();
            let input_value = input.value();
            let input_ancestors = input
                .parent_spends
                .iter()
                .map(|s| *s.unique_pubkey())
                .collect();
            let mut input_remaining_value = input_value.as_nano();
            let mut donate_to = BTreeMap::new();

            // take value from input and distribute it to outputs
            while input_remaining_value > 0 {
                if let Some((output, amount, purpose)) = outputs_iter.next() {
                    let amount_to_take = min(input_remaining_value, amount.as_nano());
                    input_remaining_value -= amount_to_take;
                    let output_key = output.unique_pubkey();
                    donate_to.insert(
                        output_key,
                        (NanoTokens::from(amount_to_take), purpose.clone()),
                    );
                } else {
                    // if we run out of outputs, send the rest as change
                    let rng = &mut rand::thread_rng();
                    let change_derivation_index = DerivationIndex::random(rng);
                    let change_key = change_to.new_unique_pubkey(&change_derivation_index);
                    donate_to.insert(
                        change_key,
                        (NanoTokens::from(input_remaining_value), OutputPurpose::None),
                    );

                    // assign the change cash note
                    change_cn = Some(CashNote {
                        parent_spends: BTreeSet::new(),
                        main_pubkey: change_to,
                        derivation_index: change_derivation_index,
                    });
                    let change_amount = NanoTokens::from(input_remaining_value);
                    #[cfg(debug_assertions)]
                    assert_eq!(_theory_change_amount, change_amount);
                    donate_to.insert(change_key, (change_amount, OutputPurpose::None));
                    break;
                }
            }

            // build spend with donations computed above
            let spend = Spend {
                unique_pubkey: input_key,
                ancestors: input_ancestors,
                descendants: donate_to,
                reason: input_reason_hash.clone(),
            };
            spends.push((spend, input.derivation_index));
        }

        // return the UnsignedTransaction
        let output_cashnotes_without_spends = outputs.into_iter().map(|(cn, _, _)| cn).collect();
        Ok(Self {
            output_cashnotes_without_spends,
            change_cashnote_without_spends: change_cn,
            spends,
        })
    }

    /// Sign the `UnsignedTransaction` with the given secret key
    /// and return the `SignedTransaction`
    /// It is advised to verify the `UnsignedTransaction` before signing if it comes from an external source
    pub fn sign(self, sk: &MainSecretKey) -> Result<SignedTransaction> {
        // sign the spends
        let signed_spends: BTreeSet<SignedSpend> = self
            .spends
            .iter()
            .map(|(spend, derivation_index)| {
                let derived_sk = sk.derive_key(derivation_index);
                SignedSpend::sign(spend.clone(), &derived_sk)
            })
            .collect();

        // distribute signed spends to their respective CashNotes
        let change_cashnote = self.change_cashnote_without_spends.map(|mut cn| {
            let us = cn.unique_pubkey();
            let parent_spends = signed_spends
                .iter()
                .filter(|ss| ss.spend.descendants.keys().any(|k| k == &us))
                .cloned()
                .collect();
            cn.parent_spends = parent_spends;
            cn
        });
        let output_cashnotes = self
            .output_cashnotes_without_spends
            .into_iter()
            .map(|mut cn| {
                let us = cn.unique_pubkey();
                let parent_spends = signed_spends
                    .iter()
                    .filter(|ss| ss.spend.descendants.keys().any(|k| k == &us))
                    .cloned()
                    .collect();
                cn.parent_spends = parent_spends;
                cn
            })
            .collect();

        Ok(SignedTransaction {
            output_cashnotes,
            change_cashnote,
            spends: signed_spends,
        })
    }

    /// Verify the `UnsignedTransaction`
    /// NB TODO: Implement this
    pub fn verify(&self) -> Result<()> {
        Ok(())
    }

    /// Return the unique keys of the CashNotes that have been spent along with their amounts
    pub fn spent_unique_keys(&self) -> BTreeSet<(UniquePubkey, NanoTokens)> {
        self.spends
            .iter()
            .map(|(spend, _)| (spend.unique_pubkey, spend.amount()))
            .collect()
    }

    /// Return the unique keys of the CashNotes that have been created along with their amounts
    pub fn output_unique_keys(&self) -> BTreeSet<(UniquePubkey, NanoTokens)> {
        self.spends
            .iter()
            .flat_map(|(spend, _)| spend.descendants.iter().map(|(k, (v, _))| (*k, *v)))
            .collect()
    }

    /// Create a new `UnsignedTransaction` from a hex string
    pub fn from_hex(hex: &str) -> Result<Self> {
        let decoded_hex = hex::decode(hex).map_err(|e| {
            TransferError::TransactionSerialization(format!("Hex decode failed: {e}"))
        })?;
        let s = rmp_serde::from_slice(&decoded_hex).map_err(|e| {
            TransferError::TransactionSerialization(format!("Failed to deserialize: {e}"))
        })?;
        Ok(s)
    }

    /// Return the hex representation of the `UnsignedTransaction`
    pub fn to_hex(&self) -> Result<String> {
        Ok(hex::encode(rmp_serde::to_vec(self).map_err(|e| {
            TransferError::TransactionSerialization(format!("Failed to serialize: {e}"))
        })?))
    }
}

/// Select the necessary number of cash_notes from those that we were passed.
fn select_inputs(
    available_cash_notes: Vec<CashNote>,
    total_output_amount: NanoTokens,
) -> Result<(Vec<CashNote>, NanoTokens)> {
    let mut cash_notes_to_spend = Vec::new();
    let mut total_input_amount = NanoTokens::zero();
    let mut change_amount = total_output_amount;

    for cash_note in available_cash_notes {
        let cash_note_balance = cash_note.value();

        // Add this CashNote as input to be spent.
        cash_notes_to_spend.push(cash_note);

        // Input amount increases with the amount of the cash_note.
        total_input_amount = total_input_amount
            .checked_add(cash_note_balance)
            .ok_or(TransferError::NumericOverflow)?;

        // If we've already combined input CashNotes for the total output amount, then stop.
        match change_amount.checked_sub(cash_note_balance) {
            Some(pending_output) => {
                change_amount = pending_output;
                if change_amount.as_nano() == 0 {
                    break;
                }
            }
            None => {
                change_amount =
                    NanoTokens::from(cash_note_balance.as_nano() - change_amount.as_nano());
                break;
            }
        }
    }

    // Make sure total input amount gathered with input CashNotes are enough for the output amount
    if total_output_amount > total_input_amount {
        return Err(TransferError::NotEnoughBalance(
            total_input_amount,
            total_output_amount,
        ));
    }

    Ok((cash_notes_to_spend, change_amount))
}

#[cfg(test)]
mod tests {
    use bls::SecretKey;
    use eyre::Result;

    use super::*;

    #[test]
    fn test_unsigned_tx_serialization() -> Result<()> {
        let tx = UnsignedTransaction::new(
            vec![],
            vec![],
            MainPubkey::new(SecretKey::random().public_key()),
            SpendReason::default(),
        )?;

        let hex = tx.to_hex()?;
        let tx2 = UnsignedTransaction::from_hex(&hex)?;

        assert_eq!(tx, tx2);
        Ok(())
    }

    #[test]
    fn test_unsigned_tx_distribution() -> Result<()> {
        // NB TODO: Implement this test
        Ok(())
    }
}
