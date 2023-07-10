// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sn_protocol::error::Result;
use sn_protocol::messages::RegisterCmd;
use sn_registers::Register;

use crate::Node;

impl Node {
    /// Handle a RegisterCmd
    pub async fn handle_register_cmd(&self, cmd: &RegisterCmd) -> Result<()> {
        match cmd {
            RegisterCmd::Create {
                owner,
                name,
                tag,
                permissions,
            } => {
                let maybe_register = self.get_register_from_network(cmd.dst()).await;
                if maybe_register.is_ok() {
                    // no op, since already created
                    return Ok(());
                }
                let register = Register::new(*owner, *name, *tag, permissions.clone());
                let _ok = self.validate_and_store_register(register).await?;
                Ok(())
            }
            RegisterCmd::Edit(op) => {
                let mut register = self.get_register_from_network(cmd.dst()).await?;
                register.apply_op(op.clone())?;
                let _ok = self.validate_and_store_register(register).await?;
                Ok(())
            }
        }
    }
}
