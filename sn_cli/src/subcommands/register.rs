// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use clap::Subcommand;
use color_eyre::Result;
use sn_client::{Client, Error as ClientError};
use xor_name::XorName;

#[derive(Subcommand, Debug)]
pub enum RegisterCmds {
    /// Create a new register with the given pet name.
    Create {
        /// The name of the register to create.
        #[clap(name = "name")]
        name: String,
    },
    Edit {
        /// The name of the register to edit.
        #[clap(name = "name")]
        name: String,
        /// The entry to add to the register.
        #[clap(name = "entry")]
        entry: String,
    },
    Get {
        /// The register pet names to get.
        #[clap(name = "names")]
        names: Vec<String>,
    },
}

pub(crate) async fn register_cmds(cmds: RegisterCmds, client: &Client) -> Result<()> {
    match cmds {
        RegisterCmds::Create { name } => create_register(name, client).await?,
        RegisterCmds::Edit { name, entry } => edit_register(name, entry, client).await?,
        RegisterCmds::Get { names } => get_registers(names, client).await?,
    }
    Ok(())
}

async fn create_register(name: String, client: &Client) -> Result<()> {
    let tag = 3006;
    let xorname = XorName::from_content(name.as_bytes());
    println!("Creating Register with '{name}' at xorname: {xorname:x} and tag {tag}");

    let _register = client.create_register(xorname, tag).await?;
    println!("Successfully created register '{name}' at {xorname:?}, {tag}!");
    Ok(())
}

async fn edit_register(name: String, entry: String, client: &Client) -> Result<()> {
    let tag = 3006;
    let xorname = XorName::from_content(name.as_bytes());
    println!("Trying to retrieve Register from {xorname:?}, {tag}");

    match client.get_register(xorname, tag).await {
        Ok(mut register) => {
            println!(
                "Successfully retrieved Register '{name}' from {}, {}!",
                register.name(),
                register.tag()
            );
            println!("Editing Register '{name}' with: {entry}");
            match register.write(entry.as_bytes()).await {
                Ok(()) => {}
                Err(ref err @ ClientError::ContentBranchDetected(ref branches)) => {
                    println!(
                        "We need to merge {} branches in Register entries: {err}",
                        branches.len()
                    );
                    register.write_merging_branches(entry.as_bytes()).await?;
                }
                Err(err) => return Err(err.into()),
            }
        }
        Err(error) => {
            println!(
                "Did not retrieve Register '{name}' from all nodes in the close group! {error}"
            )
        }
    }

    Ok(())
}

async fn get_registers(names: Vec<String>, client: &Client) -> Result<()> {
    let tag = 3006;
    for name in names {
        println!("Register name passed in via `register get` is '{name}'...");
        let xorname = XorName::from_content(name.as_bytes());

        println!("Trying to retrieve Register from {xorname:?}, {tag}");

        match client.get_register(xorname, tag).await {
            Ok(register) => println!(
                "Successfully retrieved Register '{name}' from {}, {}!",
                register.name(),
                register.tag()
            ),
            Err(error) => {
                println!(
                    "Did not retrieve Register '{name}' from all nodes in the close group! {error}"
                )
            }
        }
    }

    Ok(())
}
