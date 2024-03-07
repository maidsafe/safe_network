use std::path::{Path, PathBuf};

use color_eyre::Result;

use sn_client::protocol::storage::ChunkAddress;
use sn_client::protocol::NetworkAddress;
use sn_client::transfers::NanoTokens;
use sn_client::FilesApi;

use crate::subcommands::files::ChunkManager;

pub(crate) struct Estimator {
    chunk_manager: ChunkManager,
    files_api: FilesApi,
}

impl Estimator {
    pub(crate) fn new(chunk_manager: ChunkManager, files_api: FilesApi) -> Self {
        Self {
            chunk_manager,
            files_api,
        }
    }

    /// Estimate the upload cost of a chosen file
    pub(crate) async fn estimate_cost(
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
            tokio::spawn(async move {
                let (_peer, _cost, quote) = self
                    .files_api
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
