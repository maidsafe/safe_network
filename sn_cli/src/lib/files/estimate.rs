// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::ChunkManager;

use std::path::{Path, PathBuf};

use color_eyre::Result;

use sn_client::{
    protocol::{storage::ChunkAddress, NetworkAddress},
    transfers::NanoTokens,
    FilesApi,
};

pub struct Estimator {
    chunk_manager: ChunkManager,
    files_api: FilesApi,
}

impl Estimator {
    pub fn new(chunk_manager: ChunkManager, files_api: FilesApi) -> Self {
        Self {
            chunk_manager,
            files_api,
        }
    }

    /// Estimate the upload cost of a chosen file
    pub async fn estimate_cost(
        mut self,
        path: PathBuf,
        make_data_public: bool,
        root_dir: &Path,
    ) -> Result<()> {
        self.chunk_manager
            .chunk_path(&path, false, make_data_public)?;

        let mut estimate: u64 = 0;

        let balance = FilesApi::new(self.files_api.client().clone(), root_dir.to_path_buf())
            .wallet()?
            .balance()
            .as_nano();

        for (chunk_address, _location) in self.chunk_manager.get_chunks() {
            let c = self.files_api.clone();

            tokio::spawn(async move {
                let (_peer, _cost, quote) = c
                    .wallet()
                    .expect("estimate_cost: Wallet error.")
                    .get_store_cost_at_address(NetworkAddress::from_chunk_address(
                        ChunkAddress::new(chunk_address),
                    ))
                    .await
                    .expect("estimate_cost: Error with file.");
                quote.cost.as_nano()
            })
            .await
            .map(|nanos| estimate += nanos)
            .expect("estimate_cost: Concurrency error.");
        }

        let total = balance - estimate;

        println!("**************************************");
        println!("Your current balance: {}", NanoTokens::from(balance));
        println!("Transfer cost estimate: {}", NanoTokens::from(estimate));
        println!(
            "Your balance estimate after transfer: {}",
            NanoTokens::from(total)
        );
        println!("**************************************");

        Ok(())
    }
}
