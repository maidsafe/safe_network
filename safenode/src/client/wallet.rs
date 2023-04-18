// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sn_dbc::{Dbc, DbcIdSource, DerivedKey, PublicAddress, Token};

use crate::protocol::{
    transfers::{create_online_transfer, Outputs as TransferDetails},
    wallet::{Result, SendClient, SendWallet},
};

use super::Client;

/// A wallet client can be used to send and
/// receive tokens to/from other wallets.
pub struct WalletClient<W: SendWallet> {
    client: Client,
    wallet: W,
}

impl<W: SendWallet> WalletClient<W> {
    /// Create a new wallet client.
    pub fn new(client: Client, wallet: W) -> Self {
        Self { client, wallet }
    }

    /// Send tokens to another wallet.
    pub async fn send(&mut self, amount: Token, to: PublicAddress) -> Result<()> {
        let _dbcs = self.wallet.send(vec![(amount, to)], &self.client).await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl SendClient for Client {
    async fn send(
        &self,
        dbcs: Vec<(Dbc, DerivedKey)>,
        to: Vec<(Token, DbcIdSource)>,
        change_to: PublicAddress,
    ) -> Result<TransferDetails> {
        let transfer = create_online_transfer(dbcs, to, change_to, self).await?;

        // Upload the spends to the network:

        Ok(transfer)
    }
}
