use crate::autonomi_network_token::AutonomiNetworkTokenContract::AutonomiNetworkTokenContractInstance;
use alloy::primitives::{Address, U256};
use alloy::providers::{Network, Provider};
use alloy::sol;
use alloy::transports::Transport;
use thiserror::Error;

sol!(
    #[allow(clippy::too_many_arguments)]
    #[allow(missing_docs)]
    #[sol(rpc)]
    AutonomiNetworkTokenContract,
    "abi/AutonomiNetworkToken.json"
);

#[derive(Error, Debug)]
pub enum AutonomiNetworkTokenError {
    #[error(transparent)]
    AlloyContractError(#[from] alloy::contract::Error),
}

pub struct AutonomiNetworkToken<T: Transport + Clone, P: Provider<T, N>, N: Network> {
    contract: AutonomiNetworkTokenContractInstance<T, P, N>,
}

impl<T, P, N> AutonomiNetworkToken<T, P, N>
where
    T: Transport + Clone,
    P: Provider<T, N>,
    N: Network,
{
    pub fn new(address: Address, provider: P) -> Self {
        let contract = AutonomiNetworkTokenContract::new(address, provider);
        AutonomiNetworkToken { contract }
    }

    pub async fn balance_of(&self, account: Address) -> Result<U256, AutonomiNetworkTokenError> {
        let balance = self.contract.balanceOf(account).call().await?._0;
        Ok(balance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc_client::{EvmRpcClient, ARBITRUM_SEPOLIA_RPC_URL};
    use alloy::primitives::address;
    use alloy::primitives::utils::parse_ether;

    #[tokio::test]
    async fn test_balance_of() {
        // Using Arbitrum Sepolia
        let contract_address = address!("4bc1aCE0E66170375462cB4E6Af42Ad4D5EC689C");
        let account = address!("91537C44fF0fE61E8142976A94a3295E17Db82F3");
        let rpc_client = EvmRpcClient::new(ARBITRUM_SEPOLIA_RPC_URL).unwrap();

        let autonomi_network_token =
            AutonomiNetworkToken::new(contract_address, &rpc_client.provider);
        let balance = autonomi_network_token.balance_of(account).await.unwrap();

        assert_eq!(balance, U256::from(parse_ether("1").unwrap()));
    }
}
