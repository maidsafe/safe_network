// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    transaction::{Output, Transaction},
    DerivationIndex, DerivedSecretKey, FeeOutput, Input, MainPubkey, Spend, UniquePubkey,
};
use crate::{CashNote, Error, Hash, Nano, Result, SignedSpend};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub type InputSrcTx = Transaction;

/// A builder to create a Transaction from
/// inputs and outputs.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TransactionBuilder {
    inputs: Vec<Input>,
    outputs: Vec<Output>,
    fee: FeeOutput,
    input_details: BTreeMap<UniquePubkey, (DerivedSecretKey, InputSrcTx)>,
    output_details: BTreeMap<UniquePubkey, (MainPubkey, DerivationIndex)>,
}

impl TransactionBuilder {
    /// Add an input given a the Input, the input's derived_key and the input's src transaction
    pub fn add_input(
        mut self,
        input: Input,
        derived_key: DerivedSecretKey,
        input_src_tx: InputSrcTx,
    ) -> Self {
        self.input_details
            .insert(input.unique_pubkey(), (derived_key, input_src_tx));
        self.inputs.push(input);
        self
    }

    /// Add an input given an iterator over the Input, the input's derived_key and the input's src transaction
    pub fn add_inputs(
        mut self,
        inputs: impl IntoIterator<Item = (Input, DerivedSecretKey, InputSrcTx)>,
    ) -> Self {
        for (input, derived_key, input_src_tx) in inputs.into_iter() {
            self = self.add_input(input, derived_key, input_src_tx);
        }
        self
    }

    /// Add an input given a CashNote and its DerivedSecretKey.
    pub fn add_input_cashnote(
        mut self,
        cashnote: &CashNote,
        derived_key: &DerivedSecretKey,
    ) -> Result<Self> {
        let input_src_tx = cashnote.src_tx.clone();
        let input = Input {
            unique_pubkey: cashnote.unique_pubkey(),
            amount: cashnote.token()?,
        };
        self = self.add_input(input, derived_key.clone(), input_src_tx);
        Ok(self)
    }

    /// Add an input given a list of CashNotes and associated DerivedSecretKeys.
    pub fn add_input_cashnotes(
        mut self,
        cashnotes: &[(CashNote, DerivedSecretKey)],
    ) -> Result<Self> {
        for (cashnote, derived_key) in cashnotes.iter() {
            self = self.add_input_cashnote(cashnote, derived_key)?;
        }
        Ok(self)
    }

    /// Add an output given the token, the MainPubkey and the DerivationIndex
    pub fn add_output(
        mut self,
        token: Nano,
        main_pubkey: MainPubkey,
        derivation_index: DerivationIndex,
    ) -> Self {
        let unique_pubkey = main_pubkey.new_unique_pubkey(&derivation_index);

        self.output_details
            .insert(unique_pubkey, (main_pubkey, derivation_index));
        let output = Output::new(unique_pubkey, token.as_nano());
        self.outputs.push(output);

        self
    }

    /// Add a list of outputs given the tokens, the MainPubkey and the DerivationIndex
    pub fn add_outputs(
        mut self,
        outputs: impl IntoIterator<Item = (Nano, MainPubkey, DerivationIndex)>,
    ) -> Self {
        for (token, main_pubkey, derivation_index) in outputs.into_iter() {
            self = self.add_output(token, main_pubkey, derivation_index);
        }
        self
    }

    /// Sets the given fee output.
    pub fn set_fee_output(mut self, output: FeeOutput) -> Self {
        self.fee = output;
        self
    }

    /// Get a list of input ids.
    pub fn input_ids(&self) -> Vec<UniquePubkey> {
        self.inputs.iter().map(|i| i.unique_pubkey()).collect()
    }

    /// Get sum of inputs
    pub fn inputs_tokens_sum(&self) -> Nano {
        let amount = self.inputs.iter().map(|i| i.amount.as_nano()).sum();
        Nano::from(amount)
    }

    /// Get sum of outputs
    pub fn outputs_tokens_sum(&self) -> Nano {
        let amount = self
            .outputs
            .iter()
            .map(|o| o.amount.as_nano())
            .chain(std::iter::once(self.fee.token.as_nano()))
            .sum();
        Nano::from(amount)
    }

    /// Get inputs.
    pub fn inputs(&self) -> &Vec<Input> {
        &self.inputs
    }

    /// Get outputs.
    pub fn outputs(&self) -> &Vec<Output> {
        &self.outputs
    }

    /// Build the Transaction by signing the inputs. Return a CashNoteBuilder.
    pub fn build(self, reason: Hash) -> Result<CashNoteBuilder> {
        let spent_tx = Transaction {
            inputs: self.inputs.clone(),
            outputs: self.outputs.clone(),
            fee: self.fee.clone(),
        };
        let signed_spends: BTreeSet<_> = self
            .inputs
            .iter()
            .flat_map(|input| {
                let (derived_key, input_src_tx) = self.input_details.get(&input.unique_pubkey)?;
                let spend = Spend {
                    unique_pubkey: input.unique_pubkey(),
                    spent_tx: spent_tx.clone(),
                    reason,
                    token: input.amount,
                    cashnote_creation_tx: input_src_tx.clone(),
                };
                let derived_key_sig = derived_key.sign(&spend.to_bytes());
                Some(SignedSpend {
                    spend,
                    derived_key_sig,
                })
            })
            .collect();

        Ok(CashNoteBuilder::new(
            spent_tx,
            self.output_details,
            signed_spends,
        ))
    }
}

/// A Builder for aggregating SignedSpends and generating the final CashNote outputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashNoteBuilder {
    pub spent_tx: Transaction,
    pub output_details: BTreeMap<UniquePubkey, (MainPubkey, DerivationIndex)>,
    pub signed_spends: BTreeSet<SignedSpend>,
}

impl CashNoteBuilder {
    /// Create a new CashNoteBuilder.
    pub fn new(
        spent_tx: Transaction,
        output_details: BTreeMap<UniquePubkey, (MainPubkey, DerivationIndex)>,
        signed_spends: BTreeSet<SignedSpend>,
    ) -> Self {
        Self {
            spent_tx,
            output_details,
            signed_spends,
        }
    }

    /// Return the signed spends. They each already contain the
    /// spent_tx, so the inclusion of it in the result is just for convenience.
    pub fn signed_spends(&self) -> Vec<&SignedSpend> {
        self.signed_spends.iter().collect()
    }

    /// Build the output CashNotes, verifying the transaction and SignedSpends.
    ///
    /// See TransactionVerifier::verify() for a description of
    /// verifier requirements.
    pub fn build(self) -> Result<Vec<(CashNote, Nano)>> {
        // Verify the tx, along with signed spends.
        // Note that we do this just once for entire tx, not once per output CashNote.
        self.spent_tx
            .verify_against_inputs_spent(&self.signed_spends)?;

        // Build output CashNotes.
        self.build_output_cashnotes()
    }

    /// Build the output CashNotes (no verification over Tx or SignedSpend is performed).
    pub fn build_without_verifying(self) -> Result<Vec<(CashNote, Nano)>> {
        self.build_output_cashnotes()
    }

    // Private helper to build output CashNotes.
    fn build_output_cashnotes(self) -> Result<Vec<(CashNote, Nano)>> {
        self.spent_tx
            .outputs
            .iter()
            .map(|output| {
                let (main_pubkey, derivation_index) = self
                    .output_details
                    .get(&output.unique_pubkey)
                    .ok_or(Error::UniquePubkeyNotFound)?;

                Ok((
                    CashNote {
                        id: main_pubkey.new_unique_pubkey(derivation_index),
                        src_tx: self.spent_tx.clone(),
                        signed_spends: self.signed_spends.clone(),
                        main_pubkey: *main_pubkey,
                        derivation_index: *derivation_index,
                    },
                    output.amount,
                ))
            })
            .collect()
    }
}
