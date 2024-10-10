use crate::common::{Address, TxHash, U256};
use crate::contract::network_token::NetworkTokenContract::NetworkTokenContractInstance;
use alloy::providers::{Network, Provider};
use alloy::sol;
use alloy::transports::{RpcError, Transport, TransportErrorKind};

sol!(
    #[allow(clippy::too_many_arguments)]
    #[allow(missing_docs)]
    #[sol(rpc)]
    NetworkTokenContract,
    "artifacts/AutonomiNetworkToken.json"
);

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    ContractError(#[from] alloy::contract::Error),
    #[error(transparent)]
    RpcError(#[from] RpcError<TransportErrorKind>),
    #[error(transparent)]
    PendingTransactionError(#[from] alloy::providers::PendingTransactionError),
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
    /// Create a new NetworkToken contract instance.
    pub fn new(contract_address: Address, provider: P) -> Self {
        let contract = NetworkTokenContract::new(contract_address, provider);
        NetworkToken { contract }
    }

    /// Deploys the AutonomiNetworkToken smart contract to the network of the provider.
    /// ONLY DO THIS IF YOU KNOW WHAT YOU ARE DOING!
    pub async fn deploy(provider: P) -> Self {
        let contract = NetworkTokenContract::deploy(provider)
            .await
            .expect("Could not deploy contract");
        NetworkToken { contract }
    }

    pub fn set_provider(&mut self, provider: P) {
        let address = *self.contract.address();
        self.contract = NetworkTokenContract::new(address, provider);
    }

    /// Get the raw token balance of an address.
    pub async fn balance_of(&self, account: Address) -> Result<U256, Error> {
        debug!("Getting balance of account: {account:?}");
        let balance = self
            .contract
            .balanceOf(account)
            .call()
            .await
            .inspect_err(|err| error!("Error getting balance of account: {err:?}"))?
            ._0;
        debug!("Balance of account: {account} is {balance}");
        Ok(balance)
    }

    /// Approve spender to spend a raw amount of tokens.
    pub async fn approve(&self, spender: Address, value: U256) -> Result<TxHash, Error> {
        debug!("Approving spender to spend raw amt of tokens: {value}");
        let call = self.contract.approve(spender, value);
        let pending_tx_builder = call.send().await.inspect_err(|err| {
            error!(
                "Error approving spender {spender:?} to spend raw amt of tokens {value}:  {err:?}"
            )
        })?;

        let pending_tx_hash = *pending_tx_builder.tx_hash();
        debug!("The approval from sender {spender:?} is pending with tx_hash: {pending_tx_hash:?}",);
        let tx_hash = pending_tx_builder.watch().await.inspect_err(|err| {
            error!("Error watching approve tx with hash {pending_tx_hash:?}:  {err:?}")
        })?;

        debug!("Approve tx with hash {tx_hash:?} is successful");

        Ok(tx_hash)
    }

    /// Transfer a raw amount of tokens.
    pub async fn transfer(&self, receiver: Address, amount: U256) -> Result<TxHash, Error> {
        debug!("Transferring raw amt of tokens: {amount} to {receiver:?}");
        let call = self.contract.transfer(receiver, amount);
        let pending_tx_builder = call.send().await.inspect_err(|err| {
            error!("Error transferring raw amt of tokens to {receiver:?}: {err:?}")
        })?;

        let pending_tx_hash = *pending_tx_builder.tx_hash();
        debug!(
            "The transfer to receiver {receiver:?} is pending with tx_hash: {pending_tx_hash:?}"
        );
        let tx_hash = pending_tx_builder.watch().await.inspect_err(|err| {
            error!("Error watching transfer tx with hash {pending_tx_hash:?}: {err:?}")
        })?;

        debug!("Transfer tx with hash {tx_hash:?} is successful");

        Ok(tx_hash)
    }
}
