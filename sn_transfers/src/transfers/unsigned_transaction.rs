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
    error::Result, CashNote, DerivationIndex, MainPubkey, MainSecretKey, NanoTokens, SignedSpend,
    SignedTransaction, Spend, SpendReason, TransferError,
};

use serde::{Deserialize, Serialize};

/// A local transaction that has not been signed yet
/// All fields are private to prevent bad useage
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct UnsignedTransaction {
    /// Output CashNotes stripped of their parent spends, unuseable as is
    output_cashnotes_without_spends: Vec<CashNote>,
    /// Change CashNote stripped of its parent spends, unuseable as is
    pub change_cashnote_without_spends: Option<CashNote>,
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
        recipients: Vec<(NanoTokens, MainPubkey, DerivationIndex, bool)>,
        change_to: MainPubkey,
        input_reason_hash: SpendReason,
    ) -> Result<Self> {
        // check output amounts (reject zeroes and overflowing values)
        let total_output_amount = recipients
            .iter()
            .try_fold(NanoTokens::zero(), |total, (amount, _, _, _)| {
                total.checked_add(*amount)
            })
            .ok_or(TransferError::ExcessiveNanoValue)?;
        if total_output_amount == NanoTokens::zero()
            || recipients
                .iter()
                .any(|(amount, _, _, _)| amount.as_nano() == 0)
        {
            return Err(TransferError::ZeroOutputs);
        }

        // check input amounts
        let total_input_amount = available_cash_notes
            .iter()
            .map(|cn| cn.value())
            .try_fold(NanoTokens::zero(), |total, amount| {
                total.checked_add(amount)
            })
            .ok_or(TransferError::ExcessiveNanoValue)?;
        if total_output_amount > total_input_amount {
            return Err(TransferError::NotEnoughBalance(
                total_input_amount,
                total_output_amount,
            ));
        }

        // create empty output cash notes for recipients
        let outputs: Vec<(CashNote, NanoTokens, bool)> = recipients
            .iter()
            .map(|(amount, main_pk, derivation_index, is_royaltiy)| {
                let cn = CashNote {
                    parent_spends: BTreeSet::new(),
                    main_pubkey: *main_pk,
                    derivation_index: *derivation_index,
                };
                (cn, *amount, *is_royaltiy)
            })
            .collect();

        // order inputs by value, re const after sorting
        let mut cashnotes_big_to_small = available_cash_notes;
        cashnotes_big_to_small.sort_by_key(|b| std::cmp::Reverse(b.value()));
        let cashnotes_big_to_small = cashnotes_big_to_small;

        // distribute value from inputs to output cash notes
        let mut spends = Vec::new();
        let mut change_cn = None;
        let mut outputs_iter = outputs.iter();
        let mut current_output = outputs_iter.next();
        let mut current_output_remaining_value = current_output
            .map(|(_, amount, _)| amount.as_nano())
            .unwrap_or(0);
        let mut no_more_outputs = false;
        for input in cashnotes_big_to_small {
            let input_key = input.unique_pubkey();
            let input_value = input.value();
            let input_ancestors = input
                .parent_spends
                .iter()
                .map(|s| *s.unique_pubkey())
                .collect();
            let mut input_remaining_value = input_value.as_nano();
            let mut donate_to = BTreeMap::new();
            let mut royalties = vec![];

            // take value from input and distribute it to outputs
            while input_remaining_value > 0 {
                if let Some((output, _, is_royalty)) = current_output {
                    // give as much as possible to the current output
                    let amount_to_take = min(input_remaining_value, current_output_remaining_value);
                    input_remaining_value -= amount_to_take;
                    current_output_remaining_value -= amount_to_take;
                    let output_key = output.unique_pubkey();
                    donate_to.insert(output_key, NanoTokens::from(amount_to_take));
                    if *is_royalty {
                        royalties.push(output.derivation_index);
                    }

                    // move to the next output if the current one is fully funded
                    if current_output_remaining_value == 0 {
                        current_output = outputs_iter.next();
                        current_output_remaining_value = current_output
                            .map(|(_, amount, _)| amount.as_nano())
                            .unwrap_or(0);
                    }
                } else {
                    // if we run out of outputs, send the rest as change
                    let rng = &mut rand::thread_rng();
                    let change_derivation_index = DerivationIndex::random(rng);
                    let change_key = change_to.new_unique_pubkey(&change_derivation_index);
                    donate_to.insert(change_key, NanoTokens::from(input_remaining_value));

                    // assign the change cash note
                    change_cn = Some(CashNote {
                        parent_spends: BTreeSet::new(),
                        main_pubkey: change_to,
                        derivation_index: change_derivation_index,
                    });
                    let change_amount = NanoTokens::from(input_remaining_value);
                    donate_to.insert(change_key, change_amount);
                    no_more_outputs = true;
                    break;
                }
            }

            // build spend with donations computed above
            let spend = Spend {
                unique_pubkey: input_key,
                ancestors: input_ancestors,
                descendants: donate_to,
                reason: input_reason_hash.clone(),
                royalties,
            };
            spends.push((spend, input.derivation_index));

            // if we run out of outputs, we don't need to use all the inputs
            if no_more_outputs {
                break;
            }
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
    pub fn verify(&self) -> Result<()> {
        // verify that the tx is balanced
        let input_sum: u64 = self
            .spends
            .iter()
            .map(|(spend, _)| spend.amount().as_nano())
            .sum();
        let output_sum: u64 = self
            .output_cashnotes_without_spends
            .iter()
            .chain(self.change_cashnote_without_spends.iter())
            .map(|cn| cn.value().as_nano())
            .sum();
        if input_sum != output_sum {
            return Err(TransferError::InvalidUnsignedTransaction(format!(
                "Unbalanced transaction: input sum: {input_sum} != output sum {output_sum}"
            )));
        }

        // verify that all spends have a unique pubkey
        let mut unique_pubkeys = BTreeSet::new();
        for (spend, _) in &self.spends {
            let u = spend.unique_pubkey;
            if !unique_pubkeys.insert(u) {
                return Err(TransferError::InvalidUnsignedTransaction(format!(
                    "Spends are not unique in this transaction, there are multiple spends for: {u}"
                )));
            }
        }

        // verify that all cash notes have a unique pubkey, distinct from spends
        for cn in self
            .output_cashnotes_without_spends
            .iter()
            .chain(self.change_cashnote_without_spends.iter())
        {
            let u = cn.unique_pubkey();
            if !unique_pubkeys.insert(u) {
                return Err(TransferError::InvalidUnsignedTransaction(
                    format!("Cash note unique pubkeys are not unique in this transaction, there are multiple outputs for: {u}"),
                ));
            }
        }

        // verify that spends refer to the outputs and that the amounts match
        let mut amounts_by_unique_pubkey = BTreeMap::new();
        for (spend, _) in &self.spends {
            for (k, v) in &spend.descendants {
                amounts_by_unique_pubkey
                    .entry(*k)
                    .and_modify(|sum| *sum += v.as_nano())
                    .or_insert(v.as_nano());
            }
        }
        for cn in self
            .output_cashnotes_without_spends
            .iter()
            .chain(self.change_cashnote_without_spends.iter())
        {
            let u = cn.unique_pubkey();
            let expected_amount = amounts_by_unique_pubkey.get(&u).copied().unwrap_or(0);
            let amount = cn.value().as_nano();
            if expected_amount != amount {
                return Err(TransferError::InvalidUnsignedTransaction(
                    format!("Invalid amount for CashNote: {u} has {expected_amount} acording to spends but self reports {amount}"),
                ));
            }
        }
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
            .flat_map(|(spend, _)| spend.descendants.iter().map(|(k, v)| (*k, *v)))
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

#[cfg(test)]
mod tests {
    use super::*;
    use eyre::{Ok, Result};

    #[test]
    fn test_unsigned_tx_serialization() -> Result<()> {
        let mut rng = rand::thread_rng();
        let cnr_sk = MainSecretKey::random();
        let cnr_pk = cnr_sk.main_pubkey();
        let cnr_di = DerivationIndex::random(&mut rng);
        let cnr_upk = cnr_pk.new_unique_pubkey(&cnr_di);
        let spend = SignedSpend::random_spend_to(&mut rng, cnr_upk, 100);

        let available_cash_notes = vec![CashNote {
            parent_spends: BTreeSet::from_iter([spend]),
            main_pubkey: cnr_pk,
            derivation_index: cnr_di,
        }];
        let recipients = vec![
            (
                NanoTokens::from(1),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
            (
                NanoTokens::from(1),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
        ];
        let change_to = MainSecretKey::random().main_pubkey();
        let input_reason_hash = Default::default();
        let tx = UnsignedTransaction::new(
            available_cash_notes,
            recipients,
            change_to,
            input_reason_hash,
        )
        .expect("UnsignedTransaction creation to succeed");
        let hex = tx.to_hex()?;
        let tx2 = UnsignedTransaction::from_hex(&hex)?;

        assert_eq!(tx, tx2);
        Ok(())
    }

    #[test]
    fn test_unsigned_tx_empty_inputs_is_rejected() -> Result<()> {
        let mut rng = rand::thread_rng();
        let available_cash_notes = vec![];
        let recipients = vec![
            (
                NanoTokens::from(1),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
            (
                NanoTokens::from(1),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
        ];
        let change_to = MainSecretKey::random().main_pubkey();
        let input_reason_hash = Default::default();
        let tx = UnsignedTransaction::new(
            available_cash_notes,
            recipients,
            change_to,
            input_reason_hash,
        );
        assert_eq!(
            tx,
            Err(TransferError::NotEnoughBalance(
                NanoTokens::zero(),
                NanoTokens::from(2)
            ))
        );
        Ok(())
    }

    #[test]
    fn test_unsigned_tx_empty_outputs_is_rejected() -> Result<()> {
        let mut rng = rand::thread_rng();
        let available_cash_notes = vec![CashNote {
            parent_spends: BTreeSet::new(),
            main_pubkey: MainSecretKey::random().main_pubkey(),
            derivation_index: DerivationIndex::random(&mut rng),
        }];
        let recipients = vec![];
        let change_to = MainSecretKey::random().main_pubkey();
        let input_reason_hash = SpendReason::default();
        let tx = UnsignedTransaction::new(
            available_cash_notes.clone(),
            recipients,
            change_to,
            input_reason_hash.clone(),
        );
        assert_eq!(tx, Err(TransferError::ZeroOutputs));
        let recipients = vec![(
            NanoTokens::zero(),
            MainSecretKey::random().main_pubkey(),
            DerivationIndex::random(&mut rng),
            false,
        )];
        let tx = UnsignedTransaction::new(
            available_cash_notes,
            recipients,
            change_to,
            input_reason_hash,
        );
        assert_eq!(tx, Err(TransferError::ZeroOutputs));
        Ok(())
    }

    #[test]
    fn test_unsigned_tx_distribution_insufficient_funds() -> Result<()> {
        let mut rng = rand::thread_rng();

        // create an input cash note of 100
        let cnr_sk = MainSecretKey::random();
        let cnr_pk = cnr_sk.main_pubkey();
        let cnr_di = DerivationIndex::random(&mut rng);
        let cnr_upk = cnr_pk.new_unique_pubkey(&cnr_di);
        let spend = SignedSpend::random_spend_to(&mut rng, cnr_upk, 100);
        let cn1 = CashNote {
            parent_spends: BTreeSet::from_iter([spend]),
            main_pubkey: cnr_pk,
            derivation_index: cnr_di,
        };

        // create an unsigned transaction
        // 100 -> 50 + 55
        let available_cash_notes = vec![cn1];
        let recipients = vec![
            (
                NanoTokens::from(50),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
            (
                NanoTokens::from(55),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
        ];
        let change_to = MainSecretKey::random().main_pubkey();
        let input_reason_hash = Default::default();
        let tx = UnsignedTransaction::new(
            available_cash_notes,
            recipients,
            change_to,
            input_reason_hash,
        );

        assert_eq!(
            tx,
            Err(TransferError::NotEnoughBalance(
                NanoTokens::from(100),
                NanoTokens::from(105)
            ))
        );
        Ok(())
    }

    #[test]
    fn test_unsigned_tx_distribution_1_to_2() -> Result<()> {
        let mut rng = rand::thread_rng();

        // create an input cash note of 100
        let cnr_sk = MainSecretKey::random();
        let cnr_pk = cnr_sk.main_pubkey();
        let cnr_di = DerivationIndex::random(&mut rng);
        let cnr_upk = cnr_pk.new_unique_pubkey(&cnr_di);
        let spend = SignedSpend::random_spend_to(&mut rng, cnr_upk, 100);
        let cn1 = CashNote {
            parent_spends: BTreeSet::from_iter([spend]),
            main_pubkey: cnr_pk,
            derivation_index: cnr_di,
        };

        // create an unsigned transaction
        // 100 -> 50 + 25 + 25 change
        let available_cash_notes = vec![cn1];
        let recipients = vec![
            (
                NanoTokens::from(50),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
            (
                NanoTokens::from(25),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
        ];
        let change_to = MainSecretKey::random().main_pubkey();
        let input_reason_hash = Default::default();
        let tx = UnsignedTransaction::new(
            available_cash_notes,
            recipients,
            change_to,
            input_reason_hash,
        )
        .expect("UnsignedTransaction creation to succeed");

        // sign the transaction
        let signed_tx = tx.sign(&cnr_sk).expect("signing to succeed");

        // verify the transaction
        signed_tx.verify().expect("verify to succeed");

        // check the output cash notes
        let output_values: BTreeSet<u64> = signed_tx
            .output_cashnotes
            .iter()
            .map(|cn| cn.value().as_nano())
            .collect();
        assert_eq!(output_values, BTreeSet::from_iter([50, 25]));
        assert_eq!(
            signed_tx
                .change_cashnote
                .as_ref()
                .expect("to have a change cashnote")
                .value()
                .as_nano(),
            25
        );
        Ok(())
    }

    #[test]
    fn test_unsigned_tx_distribution_2_to_1() -> Result<()> {
        let mut rng = rand::thread_rng();

        // create an input cash note of 50
        let cnr_sk = MainSecretKey::random();
        let cnr_pk = cnr_sk.main_pubkey();
        let cnr_di = DerivationIndex::random(&mut rng);
        let cnr_upk = cnr_pk.new_unique_pubkey(&cnr_di);
        let spend = SignedSpend::random_spend_to(&mut rng, cnr_upk, 50);
        let cn1 = CashNote {
            parent_spends: BTreeSet::from_iter([spend]),
            main_pubkey: cnr_pk,
            derivation_index: cnr_di,
        };

        // create an input cash note of 25
        let cnr_di = DerivationIndex::random(&mut rng);
        let cnr_upk = cnr_pk.new_unique_pubkey(&cnr_di);
        let spend = SignedSpend::random_spend_to(&mut rng, cnr_upk, 25);
        let cn2 = CashNote {
            parent_spends: BTreeSet::from_iter([spend]),
            main_pubkey: cnr_pk,
            derivation_index: cnr_di,
        };

        // create an unsigned transaction
        // 50 + 25 -> 75 + 0 change
        let available_cash_notes = vec![cn1, cn2];
        let recipients = vec![(
            NanoTokens::from(75),
            MainSecretKey::random().main_pubkey(),
            DerivationIndex::random(&mut rng),
            false,
        )];
        let change_to = MainSecretKey::random().main_pubkey();
        let input_reason_hash = Default::default();
        let tx = UnsignedTransaction::new(
            available_cash_notes,
            recipients,
            change_to,
            input_reason_hash,
        )
        .expect("UnsignedTransaction creation to succeed");

        // sign the transaction
        let signed_tx = tx.sign(&cnr_sk).expect("signing to succeed");

        // verify the transaction
        signed_tx.verify().expect("verify to succeed");

        // check the output cash notes
        let output_values: BTreeSet<u64> = signed_tx
            .output_cashnotes
            .iter()
            .map(|cn| cn.value().as_nano())
            .collect();
        assert_eq!(output_values, BTreeSet::from_iter([75]));
        assert_eq!(signed_tx.change_cashnote, None);
        Ok(())
    }

    #[test]
    fn test_unsigned_tx_distribution_2_to_2() -> Result<()> {
        let mut rng = rand::thread_rng();

        // create an input cash note of 50
        let cnr_sk = MainSecretKey::random();
        let cnr_pk = cnr_sk.main_pubkey();
        let cnr_di = DerivationIndex::random(&mut rng);
        let cnr_upk = cnr_pk.new_unique_pubkey(&cnr_di);
        let spend = SignedSpend::random_spend_to(&mut rng, cnr_upk, 50);
        let cn1 = CashNote {
            parent_spends: BTreeSet::from_iter([spend]),
            main_pubkey: cnr_pk,
            derivation_index: cnr_di,
        };

        // create an input cash note of 25
        let cnr_di = DerivationIndex::random(&mut rng);
        let cnr_upk = cnr_pk.new_unique_pubkey(&cnr_di);
        let spend = SignedSpend::random_spend_to(&mut rng, cnr_upk, 25);
        let cn2 = CashNote {
            parent_spends: BTreeSet::from_iter([spend]),
            main_pubkey: cnr_pk,
            derivation_index: cnr_di,
        };

        // create an unsigned transaction
        // 50 + 25 -> 10 + 60 + 5 change
        let available_cash_notes = vec![cn1, cn2];
        let recipients = vec![
            (
                NanoTokens::from(10),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
            (
                NanoTokens::from(60),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
        ];
        let change_to = MainSecretKey::random().main_pubkey();
        let input_reason_hash = Default::default();
        let tx = UnsignedTransaction::new(
            available_cash_notes,
            recipients,
            change_to,
            input_reason_hash,
        )
        .expect("UnsignedTransaction creation to succeed");

        // sign the transaction
        let signed_tx = tx.sign(&cnr_sk).expect("signing to succeed");

        // verify the transaction
        signed_tx.verify().expect("verify to succeed");

        // check the output cash notes
        let output_values: BTreeSet<u64> = signed_tx
            .output_cashnotes
            .iter()
            .map(|cn| cn.value().as_nano())
            .collect();
        assert_eq!(output_values, BTreeSet::from_iter([10, 60]));
        assert_eq!(
            signed_tx
                .change_cashnote
                .as_ref()
                .expect("to have a change cashnote")
                .value()
                .as_nano(),
            5
        );
        Ok(())
    }

    #[test]
    fn test_unsigned_tx_distribution_3_to_2() -> Result<()> {
        let mut rng = rand::thread_rng();

        // create an input cash note of 10
        let cnr_sk = MainSecretKey::random();
        let cnr_pk = cnr_sk.main_pubkey();
        let cnr_di = DerivationIndex::random(&mut rng);
        let cnr_upk = cnr_pk.new_unique_pubkey(&cnr_di);
        let spend = SignedSpend::random_spend_to(&mut rng, cnr_upk, 10);
        let cn1 = CashNote {
            parent_spends: BTreeSet::from_iter([spend]),
            main_pubkey: cnr_pk,
            derivation_index: cnr_di,
        };

        // create an input cash note of 20
        let cnr_di = DerivationIndex::random(&mut rng);
        let cnr_upk = cnr_pk.new_unique_pubkey(&cnr_di);
        let spend = SignedSpend::random_spend_to(&mut rng, cnr_upk, 20);
        let cn2 = CashNote {
            parent_spends: BTreeSet::from_iter([spend]),
            main_pubkey: cnr_pk,
            derivation_index: cnr_di,
        };

        // create an input cash note of 30
        let cnr_di = DerivationIndex::random(&mut rng);
        let cnr_upk = cnr_pk.new_unique_pubkey(&cnr_di);
        let spend = SignedSpend::random_spend_to(&mut rng, cnr_upk, 30);
        let cn3 = CashNote {
            parent_spends: BTreeSet::from_iter([spend]),
            main_pubkey: cnr_pk,
            derivation_index: cnr_di,
        };

        // create an unsigned transaction
        // 10 + 20 + 30 -> 31 + 21 + 8 change
        let available_cash_notes = vec![cn1, cn2, cn3];
        let recipients = vec![
            (
                NanoTokens::from(31),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
            (
                NanoTokens::from(21),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
        ];
        let change_to = MainSecretKey::random().main_pubkey();
        let input_reason_hash = Default::default();
        let tx = UnsignedTransaction::new(
            available_cash_notes,
            recipients,
            change_to,
            input_reason_hash,
        )
        .expect("UnsignedTransaction creation to succeed");

        // sign the transaction
        let signed_tx = tx.sign(&cnr_sk).expect("signing to succeed");

        // verify the transaction
        signed_tx.verify().expect("verify to succeed");

        // check the output cash notes
        let output_values: BTreeSet<u64> = signed_tx
            .output_cashnotes
            .iter()
            .map(|cn| cn.value().as_nano())
            .collect();
        assert_eq!(output_values, BTreeSet::from_iter([31, 21]));
        assert_eq!(
            signed_tx
                .change_cashnote
                .as_ref()
                .expect("to have a change cashnote")
                .value()
                .as_nano(),
            8
        );
        Ok(())
    }

    #[test]
    fn test_unsigned_tx_distribution_3_to_many_use_1() -> Result<()> {
        let mut rng = rand::thread_rng();

        // create an input cash note of 10
        let cnr_sk = MainSecretKey::random();
        let cnr_pk = cnr_sk.main_pubkey();
        let cnr_di = DerivationIndex::random(&mut rng);
        let cnr_upk = cnr_pk.new_unique_pubkey(&cnr_di);
        let spend = SignedSpend::random_spend_to(&mut rng, cnr_upk, 10);
        let cn1 = CashNote {
            parent_spends: BTreeSet::from_iter([spend]),
            main_pubkey: cnr_pk,
            derivation_index: cnr_di,
        };

        // create an input cash note of 120
        let cnr_di = DerivationIndex::random(&mut rng);
        let cnr_upk = cnr_pk.new_unique_pubkey(&cnr_di);
        let spend = SignedSpend::random_spend_to(&mut rng, cnr_upk, 120);
        let cn2 = CashNote {
            parent_spends: BTreeSet::from_iter([spend]),
            main_pubkey: cnr_pk,
            derivation_index: cnr_di,
        };

        // create an input cash note of 2
        let cnr_di = DerivationIndex::random(&mut rng);
        let cnr_upk = cnr_pk.new_unique_pubkey(&cnr_di);
        let spend = SignedSpend::random_spend_to(&mut rng, cnr_upk, 2);
        let cn3 = CashNote {
            parent_spends: BTreeSet::from_iter([spend]),
            main_pubkey: cnr_pk,
            derivation_index: cnr_di,
        };

        // create an unsigned transaction
        // 10(unused) + 120 + 1(unused) -> 10 + 1 + 10 + 1 + 10 + 1 + 10 + 1 + 10 + 1 + 10 + 1 + 54 change and two unused inputs
        let available_cash_notes = vec![cn1, cn2, cn3];
        let recipients = vec![
            (
                NanoTokens::from(10),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
            (
                NanoTokens::from(1),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                true,
            ),
            (
                NanoTokens::from(10),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
            (
                NanoTokens::from(1),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                true,
            ),
            (
                NanoTokens::from(10),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
            (
                NanoTokens::from(1),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                true,
            ),
            (
                NanoTokens::from(10),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
            (
                NanoTokens::from(1),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                true,
            ),
            (
                NanoTokens::from(10),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
            (
                NanoTokens::from(1),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                true,
            ),
            (
                NanoTokens::from(10),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
            (
                NanoTokens::from(1),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                true,
            ),
        ];
        let change_to = MainSecretKey::random().main_pubkey();
        let input_reason_hash = Default::default();
        let tx = UnsignedTransaction::new(
            available_cash_notes,
            recipients,
            change_to,
            input_reason_hash,
        )
        .expect("UnsignedTransaction creation to succeed");

        // sign the transaction
        let signed_tx = tx.sign(&cnr_sk).expect("signing to succeed");

        // verify the transaction
        signed_tx.verify().expect("verify to succeed");

        // check the output cash notes
        let output_values: BTreeSet<u64> = signed_tx
            .output_cashnotes
            .iter()
            .map(|cn| cn.value().as_nano())
            .collect();
        assert_eq!(
            output_values,
            BTreeSet::from_iter([10, 1, 10, 1, 10, 1, 10, 1, 10, 1, 10, 1])
        );
        assert_eq!(
            signed_tx
                .change_cashnote
                .as_ref()
                .expect("to have a change cashnote")
                .value()
                .as_nano(),
            54
        );
        assert_eq!(signed_tx.spends.len(), 1); // only used the first input
        Ok(())
    }

    #[test]
    fn test_unsigned_tx_distribution_3_to_many_use_all() -> Result<()> {
        let mut rng = rand::thread_rng();

        // create an input cash note of 10
        let cnr_sk = MainSecretKey::random();
        let cnr_pk = cnr_sk.main_pubkey();
        let cnr_di = DerivationIndex::random(&mut rng);
        let cnr_upk = cnr_pk.new_unique_pubkey(&cnr_di);
        let spend = SignedSpend::random_spend_to(&mut rng, cnr_upk, 30);
        let cn1 = CashNote {
            parent_spends: BTreeSet::from_iter([spend]),
            main_pubkey: cnr_pk,
            derivation_index: cnr_di,
        };

        // create an input cash note of 2
        let cnr_di = DerivationIndex::random(&mut rng);
        let cnr_upk = cnr_pk.new_unique_pubkey(&cnr_di);
        let spend = SignedSpend::random_spend_to(&mut rng, cnr_upk, 32);
        let cn2 = CashNote {
            parent_spends: BTreeSet::from_iter([spend]),
            main_pubkey: cnr_pk,
            derivation_index: cnr_di,
        };

        // create an input cash note of 120
        let cnr_di = DerivationIndex::random(&mut rng);
        let cnr_upk = cnr_pk.new_unique_pubkey(&cnr_di);
        let spend = SignedSpend::random_spend_to(&mut rng, cnr_upk, 33);
        let cn3 = CashNote {
            parent_spends: BTreeSet::from_iter([spend]),
            main_pubkey: cnr_pk,
            derivation_index: cnr_di,
        };

        // create an unsigned transaction
        // 30 + 32 + 33 -> 10 + 1 + 10 + 1 + 10 + 1 + 10 + 1 + 10 + 1 + 10 + 1 + 29 change
        let available_cash_notes = vec![cn1, cn2, cn3];
        let recipients = vec![
            (
                NanoTokens::from(10),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
            (
                NanoTokens::from(1),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                true,
            ),
            (
                NanoTokens::from(10),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
            (
                NanoTokens::from(1),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                true,
            ),
            (
                NanoTokens::from(10),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
            (
                NanoTokens::from(1),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                true,
            ),
            (
                NanoTokens::from(10),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
            (
                NanoTokens::from(1),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                true,
            ),
            (
                NanoTokens::from(10),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
            (
                NanoTokens::from(1),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                true,
            ),
            (
                NanoTokens::from(10),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                false,
            ),
            (
                NanoTokens::from(1),
                MainSecretKey::random().main_pubkey(),
                DerivationIndex::random(&mut rng),
                true,
            ),
        ];
        let change_to = MainSecretKey::random().main_pubkey();
        let input_reason_hash = Default::default();
        let tx = UnsignedTransaction::new(
            available_cash_notes,
            recipients,
            change_to,
            input_reason_hash,
        )
        .expect("UnsignedTransaction creation to succeed");

        // sign the transaction
        let signed_tx = tx.sign(&cnr_sk).expect("signing to succeed");

        // verify the transaction
        signed_tx.verify().expect("verify to succeed");

        // check the output cash notes
        let output_values: BTreeSet<u64> = signed_tx
            .output_cashnotes
            .iter()
            .map(|cn| cn.value().as_nano())
            .collect();
        assert_eq!(
            output_values,
            BTreeSet::from_iter([10, 1, 10, 1, 10, 1, 10, 1, 10, 1, 10, 1])
        );
        assert_eq!(
            signed_tx
                .change_cashnote
                .as_ref()
                .expect("to have a change cashnote")
                .value()
                .as_nano(),
            29
        );
        Ok(())
    }
}
