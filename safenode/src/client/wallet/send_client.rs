// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::Client;

use crate::domain::{
    client_transfers::{create_online_transfer, Outputs as TransferDetails},
    wallet::{Error, Result, SendClient},
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

        for spend_request_param in transfer.all_spend_requests.clone() {
            self.expect_closest_majority_response(spend_request_param)
                .await
                .map_err(|err| Error::CouldNotSendTokens(err.to_string()))?;
        }

        Ok(transfer)
    }
}
