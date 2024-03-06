// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{
    transaction::{Output, Transaction},
    CashNote, DerivationIndex, DerivedSecretKey, Hash, Input, MainPubkey, NanoTokens, SignedSpend,
    Spend, UniquePubkey,
};

use crate::{Result, TransferError};

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub type InputSrcTx = Transaction;

/// Unsigned Transfer
#[derive(custom_debug::Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnsignedTransfer {
    /// This is the transaction where all the below
    /// spends were made and cash_notes created.
    pub tx: Transaction,
    /// The unsigned spends with their corresponding owner's key derivation index.
    pub spends: BTreeSet<(Spend, DerivationIndex)>,
    /// The cash_note holding surplus tokens after
    /// spending the necessary input cash_notes.
    pub change_id: UniquePubkey,
    /// Information for aggregating signed spends and generating the final CashNote outputs.
    pub output_details: BTreeMap<UniquePubkey, (MainPubkey, DerivationIndex)>,
}

/// A builder to create a Transaction from
/// inputs and outputs.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TransactionBuilder {
    inputs: Vec<Input>,
    outputs: Vec<Output>,
    input_details: BTreeMap<UniquePubkey, (Option<DerivedSecretKey>, InputSrcTx, DerivationIndex)>,
    output_details: BTreeMap<UniquePubkey, (MainPubkey, DerivationIndex)>,
}

impl TransactionBuilder {
    /// Add an input given a the Input, the input's derived_key and the input's src transaction
    pub fn add_input(
        mut self,
        input: Input,
        derived_key: Option<DerivedSecretKey>,
        input_src_tx: InputSrcTx,
        derivation_index: DerivationIndex,
    ) -> Self {
        self.input_details.insert(
            *input.unique_pubkey(),
            (derived_key, input_src_tx, derivation_index),
        );
        self.inputs.push(input);
        self
    }

    /// Add an input given an iterator over the Input, the input's derived_key and the input's src transaction
    pub fn add_inputs(
        mut self,
        inputs: impl IntoIterator<Item = (Input, Option<DerivedSecretKey>, InputSrcTx, DerivationIndex)>,
    ) -> Self {
        for (input, derived_key, input_src_tx, derivation_index) in inputs.into_iter() {
            self = self.add_input(input, derived_key, input_src_tx, derivation_index);
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
            .insert(unique_pubkey, (main_pubkey, derivation_index));
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
    pub fn build(
        self,
        reason: Hash,
        network_royalties: Vec<DerivationIndex>,
    ) -> Result<CashNoteBuilder> {
        let spent_tx = Transaction {
            inputs: self.inputs,
            outputs: self.outputs,
        };
        let mut signed_spends = BTreeSet::new();
        for input in &spent_tx.inputs {
            if let Some((Some(derived_key), input_src_tx, _)) =
                self.input_details.get(&input.unique_pubkey)
            {
                let spend = Spend {
                    unique_pubkey: *input.unique_pubkey(),
                    spent_tx: spent_tx.clone(),
                    reason,
                    token: input.amount,
                    parent_tx: input_src_tx.clone(),
                    network_royalties: network_royalties.clone(),
                };
                let derived_key_sig = derived_key.sign(&spend.to_bytes());
                signed_spends.insert(SignedSpend {
                    spend,
                    derived_key_sig,
                });
            }
        }

        Ok(CashNoteBuilder::new(
            spent_tx,
            self.output_details,
            signed_spends,
        ))
    }

    /// Build the UnsignedTransfer which contains the generated (unsigned) Spends.
    pub fn build_unsigned_transfer(
        self,
        reason: Hash,
        network_royalties: Vec<DerivationIndex>,
        change_id: UniquePubkey,
    ) -> Result<UnsignedTransfer> {
        let tx = Transaction {
            inputs: self.inputs,
            outputs: self.outputs,
        };
        let mut spends = BTreeSet::new();
        for input in &tx.inputs {
            if let Some((_, input_src_tx, derivation_index)) =
                self.input_details.get(&input.unique_pubkey)
            {
                let spend = Spend {
                    unique_pubkey: *input.unique_pubkey(),
                    spent_tx: tx.clone(),
                    reason,
                    token: input.amount,
                    parent_tx: input_src_tx.clone(),
                    network_royalties: network_royalties.clone(),
                };
                spends.insert((spend, *derivation_index));
            }
        }

        Ok(UnsignedTransfer {
            tx,
            spends,
            change_id,
            output_details: self.output_details,
        })
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
    pub fn build(self) -> Result<Vec<(CashNote, NanoTokens)>> {
        // Verify the tx, along with signed spends.
        // Note that we do this just once for entire tx, not once per output CashNote.
        self.spent_tx
            .verify_against_inputs_spent(self.signed_spends.iter())?;

        // Build output CashNotes.
        self.build_output_cashnotes()
    }

    /// Build the output CashNotes (no verification over Tx or SignedSpend is performed).
    pub fn build_without_verifying(self) -> Result<Vec<(CashNote, NanoTokens)>> {
        self.build_output_cashnotes()
    }

    // Private helper to build output CashNotes.
    fn build_output_cashnotes(self) -> Result<Vec<(CashNote, NanoTokens)>> {
        self.spent_tx
            .outputs
            .iter()
            .map(|output| {
                let (main_pubkey, derivation_index) = self
                    .output_details
                    .get(&output.unique_pubkey)
                    .ok_or(TransferError::UniquePubkeyNotFound)?;

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
