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
        storage::dbc_address,
        wallet::{Error, Result, SendClient, SendWallet, VerifyingClient},
    },
    network::close_group_majority,
    protocol::messages::{Cmd, CmdResponse, Query, QueryResponse, Request, Response, SpendQuery},
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

            let responses = self
                .send_to_closest(Request::Cmd(cmd))
                .await
                .map_err(|err| Error::CouldNotSendTokens(err.to_string()))?;

            // Get all Ok results of the expected response type `Spend`.
            let ok_responses: Vec<_> = responses
                .iter()
                .flatten()
                .flat_map(|resp| {
                    if let Response::Cmd(CmdResponse::Spend(Ok(()))) = resp {
                        Some(())
                    } else {
                        println!("Spend error {resp:?}.");
                        None
                    }
                })
                .collect();

            // We require a majority of the close group to respond with Ok.
            if ok_responses.len() >= close_group_majority() {
                continue;
            } else {
                return Err(Error::CouldNotVerifyTransfer(format!(
                    "Not enough close group nodes accepted the spend. Got {}, required: {}.",
                    ok_responses.len(),
                    close_group_majority()
                )));
            }
        }

        Ok(transfer)
    }
}

#[async_trait::async_trait]
impl VerifyingClient for Client {
    async fn verify(&self, dbc: &Dbc) -> Result<()> {
        let mut received_spends = std::collections::BTreeSet::new();

        for spend in &dbc.signed_spends {
            let address = dbc_address(spend.dbc_id());
            let query = Query::Spend(SpendQuery::GetDbcSpend(address));

            let responses = self
                .send_to_closest(Request::Query(query))
                .await
                .map_err(|err| Error::CouldNotVerifyTransfer(err.to_string()))?;

            // Get all Ok results of the expected response type `GetDbcSpend`.
            let spends: Vec<_> = responses
                .iter()
                .flatten()
                .flat_map(|resp| {
                    if let Response::Query(QueryResponse::GetDbcSpend(Ok(signed_spend))) = resp {
                        Some(signed_spend.clone())
                    } else {
                        None
                    }
                })
                .collect();

            let ok_responses = spends.len();
            // As to not have a single rogue node deliver a bogus spend,
            // and thereby have us fail the check here
            // (we would have more than 1 spend in the BTreeSet), we must
            // look for a majority of the same responses, and ignore any other responses.
            if ok_responses >= close_group_majority() {
                // Majority of nodes in the close group returned an Ok response.
                use itertools::*;
                if let Some(spend) = spends
                    .into_iter()
                    .map(|x| (x, 1))
                    .into_group_map()
                    .into_iter()
                    .filter(|(_, v)| v.len() >= close_group_majority())
                    .max_by_key(|(_, v)| v.len())
                    .map(|(k, _)| k)
                {
                    // Majority of nodes in the close group returned the same spend.
                    let _ = received_spends.insert(spend);
                    continue;
                }
            }

            // The parent is not recognised by all peers in its close group.
            // Thus, the parent is not valid.
            info!("The spend could not be verified as valid: {address:?}. Not enough close group nodes accepted the spend. Got {ok_responses}, required: {}.", close_group_majority());

            // If not enough spends were gotten, we try error the first
            // error to the expected query returned from nodes.
            for resp in responses.iter().flatten() {
                if let Response::Query(QueryResponse::GetDbcSpend(result)) = resp {
                    let _ = result
                        .clone()
                        .map_err(|err| Error::CouldNotVerifyTransfer(err.to_string()))?;
                };
            }

            // If there were no success or fail to the expected query,
            // we check if there were any send errors.
            for resp in responses {
                let _ = resp.map_err(|err| Error::CouldNotVerifyTransfer(err.to_string()))?;
            }

            // If there was none of the above, then we had unexpected responses.
            return Err(Error::CouldNotVerifyTransfer("Unexpected response".into()));
        }

        if received_spends == dbc.signed_spends {
            Ok(())
        } else {
            Err(Error::CouldNotVerifyTransfer(
                "The spends in network were not the same as the ones in the DBC.".into(),
            ))
        }
    }
}
