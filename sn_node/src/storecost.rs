// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::Node;

use sn_protocol::error::{Error, Result};
use sn_transfers::Nano;

impl Node {
    /// Gets the local storecost.
    pub async fn current_storecost(&self) -> Result<Nano> {
        let cost = self
            .network
            .get_local_storecost()
            .await
            .map_err(|_| Error::GetStoreCostFailed)?;

        Ok(cost)
    }
}
