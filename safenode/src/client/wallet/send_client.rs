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
        wallet::{Error, Result, SendClient},
    },
    network::close_group_majority,
    protocol::messages::{Cmd, CmdResponse, Request, Response},
};

use sn_dbc::{Dbc, DbcIdSource, DerivedKey, PublicAddress, Token};

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
