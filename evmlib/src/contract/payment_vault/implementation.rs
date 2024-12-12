use crate::common::{Address, U256};
use alloy::network::Network;
use alloy::providers::Provider;
use alloy::sol;
use alloy::transports::Transport;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    PaymentVaultImplementation,
    "artifacts/PaymentVaultNoProxy.json"
);

/// Deploys the payment vault contract and returns the contract address
pub async fn deploy<T, P, N>(
    provider: &P,
    network_token_address: Address,
    batch_limit: U256,
) -> Address
where
    T: Transport + Clone,
    P: Provider<T, N>,
    N: Network,
{
    let contract = PaymentVaultImplementation::deploy(provider, network_token_address, batch_limit)
        .await
        .expect("Could not deploy payment vault implementation contract");

    *contract.address()
}
