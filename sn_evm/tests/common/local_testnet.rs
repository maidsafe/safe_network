use alloy::node_bindings::{Anvil, AnvilInstance};

/// Runs an Anvil node and returns its RPC url.
pub async fn start_anvil_node() -> eyre::Result<AnvilInstance> {
    // Spin up a local Anvil node.
    // Requires you to have Foundry installed: https://book.getfoundry.sh/getting-started/installation
    let anvil = Anvil::new().try_spawn()?;
    println!("Anvil running at `{}`", anvil.endpoint());
    Ok(anvil)
}
