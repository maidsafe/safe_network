// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    output_purpose::OutputPurpose, spend_reason::SpendReason, transaction::Output, CashNote,
    DerivationIndex, DerivedSecretKey, Input, MainPubkey, NanoTokens, SignedSpend, Spend,
    UniquePubkey,
};

use crate::Result;

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Unsigned Transfer
#[derive(custom_debug::Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnsignedTransfer {
    /// The unsigned spends with their corresponding owner's key derivation index.
    pub spends: BTreeSet<(Spend, DerivationIndex)>,
    /// The cash_note holding surplus tokens after
    /// spending the necessary input cash_notes.
    pub change_id: UniquePubkey,
    /// Information for aggregating signed spends and generating the final CashNote outputs.
    pub output_details: BTreeMap<UniquePubkey, (MainPubkey, DerivationIndex, NanoTokens)>,
}

/// A builder to create a Transaction from
/// inputs and outputs.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TransactionBuilder {
    inputs: Vec<Input>,
    outputs: Vec<Output>,
    input_details:
        BTreeMap<UniquePubkey, (Option<DerivedSecretKey>, DerivationIndex, UniquePubkey)>,
    output_details: BTreeMap<UniquePubkey, (MainPubkey, DerivationIndex, NanoTokens)>,
}

impl TransactionBuilder {
    /// Add an input:
    ///   the input's derived_key and derivation_index, the spend contains the input as one of its output
    pub fn add_input(
        mut self,
        input: Input,
        derived_key: Option<DerivedSecretKey>,
        derivation_index: DerivationIndex,
        input_src_spend: UniquePubkey,
    ) -> Self {
        self.input_details.insert(
            *input.unique_pubkey(),
            (derived_key, derivation_index, input_src_spend),
        );
        self.inputs.push(input);
        self
    }

    /// Add an input given an iterator over the Input, the input's derived_key and the input's src transaction
    pub fn add_inputs(
        mut self,
        inputs: impl IntoIterator<
            Item = (
                Input,
                Option<DerivedSecretKey>,
                DerivationIndex,
                UniquePubkey,
            ),
        >,
    ) -> Self {
        for (input, derived_key, derivation_index, input_src_spend) in inputs.into_iter() {
            self = self.add_input(input, derived_key, derivation_index, input_src_spend);
        }
        self
    }

    /// Add an output given the token, the MainPubkey and the DerivationIndex
    pub fn add_output(
        mut self,
        token: NanoTokens,
        main_pubkey: MainPubkey,
        derivation_index: DerivationIndex,
    ) -> Self {
        let unique_pubkey = main_pubkey.new_unique_pubkey(&derivation_index);

        self.output_details
            .insert(unique_pubkey, (main_pubkey, derivation_index, token));
        let output = Output::new(unique_pubkey, token.as_nano());
        self.outputs.push(output);

        self
    }

    /// Add a list of outputs given the tokens, the MainPubkey and the DerivationIndex
    pub fn add_outputs(
        mut self,
        outputs: impl IntoIterator<Item = (NanoTokens, MainPubkey, DerivationIndex)>,
    ) -> Self {
        for (token, main_pubkey, derivation_index) in outputs.into_iter() {
            self = self.add_output(token, main_pubkey, derivation_index);
        }
        self
    }

    /// Build the Transaction by signing the inputs. Return a CashNoteBuilder.
    pub fn build(self, reason: SpendReason) -> CashNoteBuilder {
        let mut signed_spends = BTreeSet::new();

        let mut descendants = BTreeMap::new();
        for output in self.outputs.iter() {
            // TODO: use proper OutputPurpose
            let _ = descendants.insert(
                output.unique_pubkey,
                (output.amount, OutputPurpose::default()),
            );
        }

        for input in &self.inputs {
            if let Some((Some(derived_key), _, input_src_spend)) =
                self.input_details.get(&input.unique_pubkey)
            {
                let mut ancestors = BTreeSet::new();
                let _ = ancestors.insert(*input_src_spend);

                let spend = Spend {
                    unique_pubkey: *input.unique_pubkey(),
                    reason: reason.clone(),
                    ancestors,
                    descendants: descendants.clone(),
                };
                let derived_key_sig = derived_key.sign(&spend.to_bytes_for_signing());
                signed_spends.insert(SignedSpend {
                    spend,
                    derived_key_sig,
                });
            }
        }

        CashNoteBuilder::new(self.output_details, signed_spends)
    }

    /// Build the UnsignedTransfer which contains the generated (unsigned) Spends.
    pub fn build_unsigned_transfer(
        self,
        reason: SpendReason,
        change_id: UniquePubkey,
    ) -> Result<UnsignedTransfer> {
        let mut descendants = BTreeMap::new();
        for output in self.outputs.iter() {
            // TODO: use proper OutputPurpose
            let _ = descendants.insert(
                output.unique_pubkey,
                (output.amount, OutputPurpose::default()),
            );
        }
        let mut spends = BTreeSet::new();
        for input in &self.inputs {
            if let Some((_, derivation_index, input_src_spend)) =
                self.input_details.get(&input.unique_pubkey)
            {
                let mut ancestors = BTreeSet::new();
                let _ = ancestors.insert(*input_src_spend);

                let spend = Spend {
                    unique_pubkey: *input.unique_pubkey(),
                    reason: reason.clone(),
                    ancestors,
                    descendants: descendants.clone(),
                };
                spends.insert((spend, *derivation_index));
            }
        }

        Ok(UnsignedTransfer {
            spends,
            change_id,
            output_details: self.output_details,
        })
    }
}

/// A Builder for aggregating SignedSpends and generating the final CashNote outputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashNoteBuilder {
    pub output_details: BTreeMap<UniquePubkey, (MainPubkey, DerivationIndex, NanoTokens)>,
    pub signed_spends: BTreeSet<SignedSpend>,
}

impl CashNoteBuilder {
    /// Create a new CashNoteBuilder.
    pub fn new(
        output_details: BTreeMap<UniquePubkey, (MainPubkey, DerivationIndex, NanoTokens)>,
        signed_spends: BTreeSet<SignedSpend>,
    ) -> Self {
        Self {
            output_details,
            signed_spends,
        }
    }

    /// Return the signed spends. They each already contain the
    /// spent_tx, so the inclusion of it in the result is just for convenience.
    pub fn signed_spends(&self) -> Vec<&SignedSpend> {
        self.signed_spends.iter().collect()
    }

    /// Build the output CashNotes
    pub fn build(self) -> Result<Vec<(CashNote, NanoTokens)>> {
        // Build output CashNotes.
        self.build_output_cashnotes()
    }

    /// Build the output CashNotes (no verification over Tx or SignedSpend is performed).
    pub fn build_without_verifying(self) -> Result<Vec<(CashNote, NanoTokens)>> {
        self.build_output_cashnotes()
    }

    // Private helper to build output CashNotes.
    fn build_output_cashnotes(self) -> Result<Vec<(CashNote, NanoTokens)>> {
        Ok(self
            .output_details
            .values()
            .map(|(main_pubkey, derivation_index, amount)| {
                (
                    CashNote {
                        unique_pubkey: main_pubkey.new_unique_pubkey(derivation_index),
                        parent_spends: self.signed_spends.clone(),
                        main_pubkey: *main_pubkey,
                        derivation_index: *derivation_index,
                    },
                    *amount,
                )
            })
            .collect())
    }
}
