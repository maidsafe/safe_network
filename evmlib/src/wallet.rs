use alloy::network::EthereumWallet;
use alloy::signers::local::{LocalSigner, PrivateKeySigner};

pub type EvmWallet = EthereumWallet;

pub fn random() -> EvmWallet {
    let signer: PrivateKeySigner = LocalSigner::random();
    EthereumWallet::from(signer)
}
