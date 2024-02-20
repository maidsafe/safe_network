use crate::subcommands::transfer::chunks::chunk_manager::ChunkManager;
use sn_client::{Client, FilesApi};
use sn_protocol::storage::ChunkAddress;
use sn_protocol::NetworkAddress;
use sn_transfers::NanoTokens;
use std::path::{Path, PathBuf};

/// Estimate the upload cost of a chosen file
pub(crate) async fn estimate_cost(
    path: PathBuf,
    make_data_public: bool,
    client: &Client,
    root_dir: &Path,
) -> eyre::Result<()> {
    let mut chunk_manager = ChunkManager::new(root_dir);
    chunk_manager.chunk_path(&path, false, make_data_public)?;

    let mut estimate: u64 = 0;

    let balance = FilesApi::new(client.clone(), root_dir.to_path_buf())
        .wallet()?
        .balance()
        .as_nano();

    for (chunk_address, _location) in chunk_manager.get_chunks() {
        let client_clone = client.clone();
        let root_dir_path_buf = root_dir.to_path_buf();

        tokio::spawn(async move {
            let (_peer, _cost, quote) = FilesApi::new(client_clone, root_dir_path_buf)
                .wallet()
                .expect("estimate_cost: Wallet error.")
                .get_store_cost_at_address(NetworkAddress::from_chunk_address(ChunkAddress::new(
                    chunk_address,
                )))
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
