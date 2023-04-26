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
        storage::dbc_address,
        wallet::{Error, Result, VerifyingClient},
    },
    network::close_group_majority,
    protocol::messages::{Query, QueryResponse, Request, Response, SpendQuery},
};

use sn_dbc::{Dbc, DbcId, SignedSpend};

#[async_trait::async_trait]
impl VerifyingClient for Client {
    async fn verify(&self, dbc: &Dbc) -> Result<()> {
        let mut received_spends = std::collections::BTreeSet::new();

        for spend in &dbc.signed_spends {
            let network_valid_spend = get_network_valid_spend(spend.dbc_id(), self).await?;
            let _ = received_spends.insert(network_valid_spend);
        }

        // If all the spends in the dbc are the same as the ones in the network,
        // we have successfully verified that the dbc is globally recognised and therefor valid.
        if received_spends == dbc.signed_spends {
            Ok(())
        } else {
            Err(Error::CouldNotVerifyTransfer(
                "The spends in network were not the same as the ones in the DBC.".into(),
            ))
        }
    }
}

async fn get_network_valid_spend(dbc_id: &DbcId, client: &Client) -> Result<SignedSpend> {
    let address = dbc_address(dbc_id);
    let query = Query::Spend(SpendQuery::GetDbcSpend(address));

    let responses = client
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
        let majority_agreement = spends
            .into_iter()
            .map(|x| (x, 1))
            .into_group_map()
            .into_iter()
            .filter(|(_, v)| v.len() >= close_group_majority())
            .max_by_key(|(_, v)| v.len())
            .map(|(k, _)| k);

        if let Some(signed_spend) = majority_agreement {
            // Majority of nodes in the close group returned the same spend.
            // We return the spend, so that it can be compared to the spends we have in the DBC.
            return Ok(signed_spend);
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
    Err(Error::CouldNotVerifyTransfer("Unexpected response".into()))
}
