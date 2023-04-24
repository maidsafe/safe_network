// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::Client;

use crate::{
    domain::{
        client_transfers::{
            create_online_transfer, Outputs as TransferDetails, SpendRequestParams,
        },
        wallet::{Error, Result, SendClient, SendWallet},
    },
    protocol::messages::{Cmd, Request},
};

use sn_dbc::{Dbc, DbcIdSource, DerivedKey, PublicAddress, Token};

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
    pub async fn send(&mut self, amount: Token, to: PublicAddress) -> Result<Dbc> {
        let dbcs = self.wallet.send(vec![(amount, to)], &self.client).await?;
        match &dbcs[..] {
            [info, ..] => Ok(info.dbc.clone()),
            [] => Err(Error::CouldNotSendTokens(
                "No DBCs were returned from the wallet.".into(),
            )),
        }
    }

    /// Return the wallet.
    pub fn into_wallet(self) -> W {
        self.wallet
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

        for spend_request_params in transfer.all_spend_request_params.clone() {
            let SpendRequestParams {
                signed_spend,
                parent_tx,
                fee_ciphers,
            } = spend_request_params;

            let cmd = Cmd::SpendDbc {
                signed_spend: Box::new(signed_spend),
                parent_tx: Box::new(parent_tx),
                fee_ciphers,
            };

            let _responses = self
                .send_to_closest(Request::Cmd(cmd))
                .await
                .map_err(|err| Error::CouldNotSendTokens(err.to_string()))?;

            // TODO: validate responses
        }

        Ok(transfer)
    }
}
