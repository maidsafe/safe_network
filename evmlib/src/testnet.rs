use crate::common::Address;
use crate::contract::chunk_payments::ChunkPayments;
use crate::contract::network_token::NetworkToken;
use crate::{CustomNetwork, Network};
use alloy::hex::ToHexExt;
use alloy::network::{Ethereum, EthereumWallet};
use alloy::node_bindings::{Anvil, AnvilInstance};
use alloy::providers::fillers::{FillProvider, JoinFill, RecommendedFiller, WalletFiller};
use alloy::providers::{ProviderBuilder, ReqwestProvider};
use alloy::signers::local::PrivateKeySigner;
use alloy::transports::http::{Client, Http};

pub struct Testnet {
    anvil: AnvilInstance,
    network_token_address: Address,
    chunk_payments_address: Address,
}

impl Testnet {
    /// Starts an Anvil node and automatically deploys the network token and chunk payments smart contracts.
    pub async fn new(royalties_wallet: Address) -> Self {
        let anvil = start_node();

        let network_token = deploy_network_token_contract(&anvil).await;
        let chunk_payments = deploy_chunk_payments_contract(
            &anvil,
            *network_token.contract.address(),
            royalties_wallet,
        )
        .await;

        Testnet {
            anvil,
            network_token_address: *network_token.contract.address(),
            chunk_payments_address: *chunk_payments.contract.address(),
        }
    }

    pub fn to_network(&self) -> Network {
        let rpc_url = self
            .anvil
            .endpoint()
            .parse()
            .expect("Could not parse RPC URL");

        Network::Custom(CustomNetwork {
            rpc_url_http: rpc_url,
            payment_token_address: self.network_token_address,
            chunk_payments_address: self.chunk_payments_address,
        })
    }

    pub fn default_wallet_private_key(&self) -> String {
        // Fetches private key from the first default Anvil account (Alice).
        let signer: PrivateKeySigner = self.anvil.keys()[0].clone().into();
        signer.to_bytes().encode_hex_with_prefix()
    }
}

/// Runs a local Anvil node.
pub fn start_node() -> AnvilInstance {
    // Spin up a local Anvil node.
    // Requires you to have Foundry installed: https://book.getfoundry.sh/getting-started/installation
    Anvil::new()
        .try_spawn()
        .expect("Could not spawn Anvil node")
}

pub async fn deploy_network_token_contract(
    anvil: &AnvilInstance,
) -> NetworkToken<
    Http<Client>,
    FillProvider<
        JoinFill<RecommendedFiller, WalletFiller<EthereumWallet>>,
        ReqwestProvider,
        Http<Client>,
        Ethereum,
    >,
    Ethereum,
> {
    // Set up signer from the first default Anvil account (Alice).
    let signer: PrivateKeySigner = anvil.keys()[0].clone().into();
    let wallet = EthereumWallet::from(signer);

    let rpc_url = anvil.endpoint().parse().expect("Could not parse RPC URL");

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet)
        .on_http(rpc_url);

    // Deploy the contract.
    NetworkToken::deploy(provider).await
}

pub async fn deploy_chunk_payments_contract(
    anvil: &AnvilInstance,
    token_address: Address,
    royalties_wallet: Address,
) -> ChunkPayments<
    Http<Client>,
    FillProvider<
        JoinFill<RecommendedFiller, WalletFiller<EthereumWallet>>,
        ReqwestProvider,
        Http<Client>,
        Ethereum,
    >,
    Ethereum,
> {
    // Set up signer from the second default Anvil account (Bob).
    let signer: PrivateKeySigner = anvil.keys()[1].clone().into();
    let wallet = EthereumWallet::from(signer);

    let rpc_url = anvil.endpoint().parse().expect("Could not parse RPC URL");

    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet)
        .on_http(rpc_url);

    // Deploy the contract.
    ChunkPayments::deploy(provider, token_address, royalties_wallet).await
}
