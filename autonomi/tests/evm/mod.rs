use autonomi;

#[cfg(feature = "files")]
mod file;
#[cfg(feature = "data")]
mod put;
#[cfg(feature = "registers")]
mod register;
mod wallet;

pub type Client = autonomi::evm::client::EvmClient;
