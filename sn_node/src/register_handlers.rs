// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sn_protocol::error::Result;
use sn_protocol::messages::{QueryResponse, RegisterCmd, RegisterQuery};

use crate::Node;

impl Node {
    /// Handle a RegisterQuery
    pub async fn handle_register_query(&self, query: &RegisterQuery) -> QueryResponse {
        let register = match self.get_register_from_network(query.dst()).await {
            Ok(reg) => reg,
            Err(e) => return QueryResponse::GetRegister(Err(e)),
        };

        match query {
            RegisterQuery::Get(_) => QueryResponse::GetRegister(Ok(register)),
            RegisterQuery::GetEntry { address: _, hash } => {
                let entry = register.get_cloned(*hash).map_err(|e| e.into());
                QueryResponse::GetRegisterEntry(entry)
            }
            RegisterQuery::GetOwner(_) => {
                let owner = register.owner();
                QueryResponse::GetRegisterOwner(Ok(owner))
            }
            RegisterQuery::Read(_) => {
                let entries = register.read();
                QueryResponse::ReadRegister(Ok(entries))
            }
            RegisterQuery::GetPolicy(_) => {
                let policy = register.policy().clone();
                QueryResponse::GetRegisterPolicy(Ok(policy))
            }
            RegisterQuery::GetUserPermissions { address: _, user } => {
                let permissions = register.permissions(*user).map_err(|e| e.into());
                QueryResponse::GetRegisterUserPermissions(permissions)
            }
        }
    }

    /// Handle a RegisterCmd
    pub async fn handle_register_cmd(&self, cmd: &RegisterCmd) -> Result<()> {
        let _res = self.get_register_from_network(cmd.dst()).await;

        Ok(())
    }
}
