// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::Client;

use futures::future::join_all;
use sn_dbc::Dbc;
use sn_transfers::wallet::{Error, Result, VerifyingClient};

#[async_trait::async_trait]
impl VerifyingClient for Client {
    async fn verify(&self, dbc: &Dbc) -> Result<()> {
        // We need to get all the spends in the dbc from the network,
        // and compare them to the spends in the dbc, to know if the
        // transfer is considered valid in the network.
        let mut tasks = Vec::new();
        for spend in &dbc.signed_spends {
            tasks.push(self.expect_closest_majority_same(spend.dbc_id()));
        }

        let mut received_spends = std::collections::BTreeSet::new();
        for result in join_all(tasks).await {
            let network_valid_spend =
                result.map_err(|err| Error::CouldNotVerifyTransfer(err.to_string()))?;
            let _ = received_spends.insert(network_valid_spend);
        }

        // If all the spends in the dbc are the same as the ones in the network,
        // we have successfully verified that the dbc is globally recognised and therefor valid.
        if received_spends == dbc.signed_spends {
            return Ok(());
        }
        Err(Error::CouldNotVerifyTransfer(
            "The spends in network were not the same as the ones in the DBC.".into(),
        ))
    }
}
