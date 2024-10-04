use clap::Subcommand;
use sn_evm::{EvmNetwork, EvmNetworkCustom};

#[derive(Subcommand, Clone, Debug)]
pub enum EvmNetworkCommand {
    /// Use the Arbitrum One network
    EvmArbitrumOne,

    /// Use a custom network
    EvmCustom {
        /// The RPC URL for the custom network
        #[arg(long)]
        rpc_url: String,

        /// The payment token contract address
        #[arg(long, short)]
        payment_token_address: String,

        /// The chunk payments contract address
        #[arg(long, short)]
        data_payments_address: String,
    },
}

#[allow(clippy::from_over_into)]
impl Into<EvmNetwork> for EvmNetworkCommand {
    fn into(self) -> EvmNetwork {
        match self {
            Self::EvmArbitrumOne => EvmNetwork::ArbitrumOne,
            Self::EvmCustom {
                rpc_url,
                payment_token_address,
                data_payments_address,
            } => EvmNetwork::Custom(EvmNetworkCustom::new(
                &rpc_url,
                &payment_token_address,
                &data_payments_address,
            )),
        }
    }
}
