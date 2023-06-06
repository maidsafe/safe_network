// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::MerkleTreeNodesType;

use merkletree::hash::Algorithm;
use tiny_keccak::{Hasher, Sha3};

// Hasher used to build the payment proof binary Merkle-tree
pub(super) struct Sha256Hasher {
    engine: Sha3,
}

impl Default for Sha256Hasher {
    fn default() -> Self {
        Self {
            engine: Sha3::v256(),
        }
    }
}

impl std::hash::Hasher for Sha256Hasher {
    fn finish(&self) -> u64 {
        // merkletree::Algorithm trait is not calling this as per its doc:
        // https://docs.rs/merkletree/latest/merkletree/hash/trait.Algorithm.html
        error!(
            "Hasher's contract (finish function is supposedly not used) is deliberately broken by design"
        );
        0
    }

    fn write(&mut self, bytes: &[u8]) {
        self.engine.update(bytes)
    }
}

impl Algorithm<MerkleTreeNodesType> for Sha256Hasher {
    fn hash(&mut self) -> MerkleTreeNodesType {
        let sha3 = self.engine.clone();
        let mut hash = MerkleTreeNodesType::default();
        sha3.finalize(&mut hash);
        hash
    }
}
