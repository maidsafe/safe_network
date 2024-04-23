// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use bls::PublicKey;
use clap::Subcommand;
use color_eyre::{eyre::WrapErr, Result, Section};
use sn_client::acc_packet::load_account_wallet_or_create_with_mnemonic;
use sn_client::protocol::storage::RegisterAddress;
use sn_client::registers::Permissions;

use sn_client::{Client, Error as ClientError, WalletClient};
use std::path::Path;
use xor_name::XorName;

#[derive(Subcommand, Debug)]
pub enum RegisterCmds {
    /// Create a new register with a name.
    Create {
        /// The name of the register to create. This could be the app's name.
        /// This is used along with your public key to derive the address of the register
        #[clap(name = "name", short = 'n')]
        name: String,

        /// Create the register with public write access.
        /// By default only the owner can write to the register.
        #[clap(name = "public", short = 'p')]
        public: bool,
    },
    Edit {
        /// The address of the register to edit.
        #[clap(name = "address")]
        address: String,
        /// If you are the owner, the name of the register can be used as a shorthand to the address,
        /// as we can derive the address from the public key + name
        /// Use this flag if you are providing the register name instead of the address
        #[clap(name = "name", short = 'n')]
        use_name: bool,
        /// The entry to add to the register.
        #[clap(name = "entry")]
        entry: String,
    },
    Get {
        /// The register addresses to get.
        #[clap(name = "addresses")]
        addresses: Vec<String>,
        /// If you are the owner, the name of the register can be used as a shorthand to the address,
        /// as we can derive the address from the public key + name
        /// Use this flag if you are providing the register names instead of the addresses
        #[clap(name = "name", short = 'n')]
        use_name: bool,
    },
}

pub(crate) async fn register_cmds(
    cmds: RegisterCmds,
    client: &Client,
    root_dir: &Path,
    verify_store: bool,
) -> Result<()> {
    match cmds {
        RegisterCmds::Create { name, public } => {
            create_register(name, public, client, root_dir, verify_store).await?
        }
        RegisterCmds::Edit {
            address,
            use_name,
            entry,
        } => edit_register(address, use_name, entry, client, verify_store).await?,
        RegisterCmds::Get {
            addresses,
            use_name,
        } => get_registers(addresses, use_name, client).await?,
    }
    Ok(())
}

async fn create_register(
    name: String,
    public: bool,
    client: &Client,
    root_dir: &Path,
    verify_store: bool,
) -> Result<()> {
    trace!("Starting to pay for Register storage");

    let wallet = load_account_wallet_or_create_with_mnemonic(root_dir, None)
        .wrap_err("Unable to read wallet file in {path:?}")
        .suggestion(
            "If you have an old wallet file, it may no longer be compatible. Try removing it",
        )?;

    let mut wallet_client = WalletClient::new(client.clone(), wallet);

    let meta = XorName::from_content(name.as_bytes());
    let perms = match public {
        true => Permissions::new_anyone_can_write(),
        false => Permissions::default(),
    };
    let (register, storage_cost, royalties_fees) = client
        .create_and_pay_for_register(meta, &mut wallet_client, verify_store, perms)
        .await?;

    if storage_cost.is_zero() {
        println!("Register '{name}' already exists!",);
    } else {
        println!(
            "Successfully created register '{name}' for {storage_cost:?} (royalties fees: {royalties_fees:?})!",
        );
    }

    println!("REGISTER_ADDRESS={}", register.address().to_hex());

    Ok(())
}

async fn edit_register(
    address_str: String,
    use_name: bool,
    entry: String,
    client: &Client,
    verify_store: bool,
) -> Result<()> {
    let (address, printing_name) = parse_addr(&address_str, use_name, client.signer_pk())?;

    println!("Trying to retrieve Register from {address}");

    match client.get_register(address).await {
        Ok(mut register) => {
            println!("Successfully retrieved Register {printing_name}",);
            println!("Editing Register {printing_name} with: {entry}");
            match register.write_online(entry.as_bytes(), verify_store).await {
                Ok(()) => {}
                Err(ref err @ ClientError::ContentBranchDetected(ref branches)) => {
                    println!(
                        "We need to merge {} branches in Register entries: {err}",
                        branches.len()
                    );
                    register
                        .write_merging_branches_online(entry.as_bytes(), verify_store)
                        .await?;
                }
                Err(err) => return Err(err.into()),
            }
        }
        Err(error) => {
            println!(
                "Did not retrieve Register {printing_name} from all nodes in the close group! {error}"
            );
            return Err(error.into());
        }
    }

    Ok(())
}

async fn get_registers(addresses: Vec<String>, use_name: bool, client: &Client) -> Result<()> {
    for addr in addresses {
        let (address, printing_name) = parse_addr(&addr, use_name, client.signer_pk())?;

        println!("Trying to retrieve Register {printing_name}");

        match client.get_register(address).await {
            Ok(register) => {
                println!("Successfully retrieved Register {printing_name}");
                let entries = register.read();
                println!("Register entries:");
                for (hash, bytes) in entries {
                    let data_str = match String::from_utf8(bytes.clone()) {
                        Ok(data_str) => data_str,
                        Err(_) => format!("{bytes:?}"),
                    };
                    println!("{hash:?}: {data_str}");
                }
            }
            Err(error) => {
                println!(
                    "Did not retrieve Register {printing_name} from all nodes in the close group! {error}"
                );
                return Err(error.into());
            }
        }
    }

    Ok(())
}

/// Parse str and return the address and the register info for printing
fn parse_addr(
    address_str: &str,
    use_name: bool,
    pk: PublicKey,
) -> Result<(RegisterAddress, String)> {
    if use_name {
        debug!("Parsing address as name");
        let user_metadata = XorName::from_content(address_str.as_bytes());
        let addr = RegisterAddress::new(user_metadata, pk);
        Ok((addr, format!("'{address_str}' at {addr}")))
    } else {
        debug!("Parsing address as hex");
        let addr = RegisterAddress::from_hex(address_str)
            .wrap_err("Could not parse hex string")
            .suggestion(
                "If getting a register by name, use the `-n` flag eg:\n
        safe register get -n <register-name>",
            )?;
        Ok((addr, format!("at {address_str}")))
    }
}
