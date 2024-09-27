use crate::client::{Client, ClientWrapper, ConnectError};
use crate::Multiaddr;

#[cfg(feature = "data")]
pub mod data;
#[cfg(feature = "files")]
pub mod files;
#[cfg(feature = "registers")]
pub mod registers;
mod vault;

#[derive(Clone)]
pub struct EvmClient {
    client: Client,
}

impl ClientWrapper for EvmClient {
    fn from_client(client: Client) -> Self {
        EvmClient { client }
    }

    fn client(&self) -> &Client {
        &self.client
    }

    fn client_mut(&mut self) -> &mut Client {
        &mut self.client
    }

    fn into_client(self) -> Client {
        self.client
    }
}
