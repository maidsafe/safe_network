use alloy::node_bindings::{Anvil, AnvilInstance};

#[allow(clippy::unwrap_used)]
/// Runs an Anvil node and returns its RPC url.
pub async fn start_anvil_node() -> AnvilInstance {
    // Spin up a local Anvil node.
    // Requires you to have Foundry installed: https://book.getfoundry.sh/getting-started/installation
    let anvil = Anvil::new().try_spawn().unwrap();
    println!("Anvil running at `{}`", anvil.endpoint());
    anvil
}
