// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::GenesisMaterial;
use crate::{mock, CashNote, Hash, NanoTokens, Result, TransactionBuilder};

/// A builder for initializing a set of N spentbooks and generating a
/// genesis cashnote with amount Z.
///
/// In SafeNetwork terms, the set of SpentBooksNodes represents a
/// single Spentbook section.
#[derive(Default)]
pub struct GenesisBuilder {
    pub spentbook_nodes: Vec<mock::SpentbookNode>,
}

impl GenesisBuilder {
    /// Generates a list of spentbook nodes and adds to the builder.
    pub fn gen_spentbook_nodes(mut self, num_nodes: usize) -> Self {
        for _ in 0..num_nodes {
            self.spentbook_nodes.push(mock::SpentbookNode::default());
        }
        self
    }

    /// Adds an existing spentbook node to the builder.
    pub fn add_spentbook_node(mut self, spentbook_node: mock::SpentbookNode) -> Self {
        self.spentbook_nodes.push(spentbook_node);
        self
    }

    /// builds and returns spentbooks, genesis_cashnote_shares, and genesis cashnote
    #[allow(clippy::type_complexity)]
    pub fn build(
        mut self,
    ) -> Result<(
        Vec<mock::SpentbookNode>,
        CashNote,
        GenesisMaterial,
        NanoTokens,
    )> {
        let genesis_material = GenesisMaterial::default();
        let cashnote_builder = TransactionBuilder::default()
            .add_input(
                genesis_material.genesis_tx.0.inputs[0].clone(),
                genesis_material.genesis_tx.1.clone(),
                genesis_material.genesis_tx.2.clone(),
            )
            .add_output(
                genesis_material.genesis_tx.0.outputs[0].amount,
                genesis_material.main_key.main_pubkey(),
                genesis_material.derivation_index,
            )
            .build(Hash::default())?;

        let tx = &cashnote_builder.spent_tx;
        for signed_spend in cashnote_builder.signed_spends() {
            for spentbook_node in self.spentbook_nodes.iter_mut() {
                spentbook_node.log_spent(tx, signed_spend)?;
            }
        }

        let (genesis_cashnote, token) = cashnote_builder.build()?.into_iter().next().unwrap();
        Ok((
            self.spentbook_nodes,
            genesis_cashnote,
            genesis_material,
            token,
        ))
    }

    /// builds and returns spentbooks, genesis_cashnote_shares, and genesis cashnote
    /// the spentbook nodes use a shared randomly generated SecretKeySet
    #[allow(clippy::type_complexity)]
    pub fn init_genesis(
        num_spentbook_nodes: usize,
    ) -> Result<(
        Vec<mock::SpentbookNode>,
        CashNote,
        GenesisMaterial,
        NanoTokens,
    )> {
        Self::default()
            .gen_spentbook_nodes(num_spentbook_nodes)
            .build()
    }

    /// Builds and returns a single spentbook, single genesis_cashnote_shares,
    /// and genesis cashnote.
    /// The spentbook node uses a shared randomly generated SecretKeySet.
    #[allow(clippy::type_complexity)]
    pub fn init_genesis_single(
    ) -> Result<(mock::SpentbookNode, CashNote, GenesisMaterial, NanoTokens)> {
        let (spentbook_nodes, genesis_cashnote, genesis_material, token) =
            Self::default().gen_spentbook_nodes(1).build()?;

        // Note: these unwraps are safe because the above call returned Ok.
        // We could (stylistically) avoid the unwrap eg spentbook_nodes[0].clone()
        // but this is more expensive and it would panic anyway if spentbook_nodes is empty.
        // For library code we would go further, but this is a  for testing,
        // so not worth making a never-used Error variant.
        Ok((
            spentbook_nodes.into_iter().next().unwrap(),
            genesis_cashnote,
            genesis_material,
            token,
        ))
    }
}
