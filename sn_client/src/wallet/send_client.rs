// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::Client;

use sn_transfers::{
    client_transfers::Outputs as TransferDetails,
    wallet::{Error, Result, SendClient},
};

use futures::future::join_all;

#[async_trait::async_trait]
impl SendClient for Client {
    async fn send(&self, transfer: TransferDetails) -> Result<()> {
        let mut tasks = Vec::new();
        for spend_request in &transfer.all_spend_requests {
            tasks.push(self.network_store_spend(spend_request.clone()));
        }

        for spend_attempt_result in join_all(tasks).await {
            spend_attempt_result.map_err(|err| Error::CouldNotSendTokens(err.to_string()))?;
        }

        Ok(())
    }
}
