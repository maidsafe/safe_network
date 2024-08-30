use crate::contract::network_token::NetworkTokenContract::NetworkTokenContractInstance;
use alloy::primitives::{Address, TxHash, U256};
use alloy::providers::{Network, Provider};
use alloy::sol;
use alloy::transports::{RpcError, Transport, TransportErrorKind};
use thiserror::Error;

sol!(
    #[allow(clippy::too_many_arguments)]
    #[allow(missing_docs)]
    #[sol(rpc)]
    NetworkTokenContract,
    "artifacts/AutonomiNetworkToken.json"
);

#[derive(Error, Debug)]
pub enum NetworkTokenError {
    #[error(transparent)]
    ContractError(#[from] alloy::contract::Error),
    #[error(transparent)]
    RpcError(#[from] RpcError<TransportErrorKind>),
}

pub struct NetworkToken<T: Transport + Clone, P: Provider<T, N>, N: Network> {
    pub contract: NetworkTokenContractInstance<T, P, N>,
}

impl<T, P, N> NetworkToken<T, P, N>
where
    T: Transport + Clone,
    P: Provider<T, N>,
    N: Network,
{
    pub fn new(network: crate::Network, provider: P) -> Self {
        let contract = NetworkTokenContract::new(*network.network_token_address(), provider);
        NetworkToken { contract }
    }

    /// Deploys the AutonomiNetworkToken smart contract to the network of the provider.
    pub async fn deploy(provider: P) -> Self {
        let contract = NetworkTokenContract::deploy(provider)
            .await
            .expect("Could not deploy contract");
        NetworkToken { contract }
    }

    pub fn set_network(&mut self, network: crate::Network) {
        self.contract.set_address(*network.network_token_address());
    }

    pub fn set_provider(&mut self, provider: P) {
        let address = *self.contract.address();
        self.contract = NetworkTokenContract::new(address, provider);
    }

    pub async fn balance_of(&self, account: Address) -> Result<U256, NetworkTokenError> {
        let balance = self.contract.balanceOf(account).call().await?._0;
        Ok(balance)
    }

    pub async fn approve(
        &self,
        spender: Address,
        value: U256,
    ) -> Result<TxHash, NetworkTokenError> {
        let tx_hash = self
            .contract
            .approve(spender, value)
            .send()
            .await?
            .watch()
            .await?;

        Ok(tx_hash)
    }
}
