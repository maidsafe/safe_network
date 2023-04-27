// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use safenode::client::{Client, ClientEvent, Error};

use bls::SecretKey;
use clap::Parser;
use eyre::Result;
use std::{io, time::Duration};
use tokio::time::sleep;
use xor_name::XorName;

#[derive(Parser, Debug)]
#[clap(name = "registers cli")]
struct Opt {
    #[clap(long)]
    user: String,

    #[clap(long)]
    reg_nickname: String,

    #[clap(long, default_value_t = 2000)]
    delay_millis: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::parse();
    let user = opt.user;
    let reg_nickname = opt.reg_nickname;
    let delay = Duration::from_millis(opt.delay_millis);

    // let's build a random secret key to sign our Register ops
    let signer = SecretKey::random();

    println!("Starting SAFE client...");
    let client = Client::new(signer, None)?;
    println!("SAFE client signer public key: {:?}", client.signer_pk());

    // Let's wait till we are connected to the network before proceeding further
    let mut client_events_rx = client.events_channel();
    loop {
        if let Ok(ClientEvent::ConnectedToNetwork) = client_events_rx.recv().await {
            println!("Connected to the Network!");
            println!();
            break;
        }
    }

    // we'll retrieve (or create if not found) a Register, and write on it
    // in offline mode, syncing with the network periodically.
    let tag = 5000;
    let xorname = XorName::from_content(reg_nickname.as_bytes());
    println!("Retrieving Register '{reg_nickname}' from SAFE, as user '{user}'");
    let mut reg_replica = match client.get_register(xorname, tag).await {
        Ok(register) => {
            println!(
                "Register '{reg_nickname}' found at {}, {}!",
                register.name(),
                register.tag(),
            );
            register.offline()
        }
        Err(_) => {
            println!("Register '{reg_nickname}' not found, creating it at {xorname}, {tag}",);
            client.create_register(xorname, tag).await?.offline()
        }
    };
    println!("Register owned by: {:?}", reg_replica.policy().owner());
    println!(
        "Register permissios: {:?}",
        reg_replica.policy().permissions
    );

    // We'll loop asking for new msg to write onto the Register offline,
    // then we'll be syncing the offline Register with the network, i.e.
    // both pushing and ulling all changes made to it by us and other clients/users.
    // If we detect branches when trying to write, after we synced with remote
    // replicas of the Register, we'll merge them all back into a single value.
    loop {
        println!();
        println!(
            "Current total number of items in Register: {}",
            reg_replica.size()
        );
        println!("Latest value (more than one if concurrent writes were made):");
        println!("--------------");
        for (_, entry) in reg_replica.read().into_iter() {
            println!("{}", String::from_utf8(entry)?);
        }
        println!("--------------");

        let input_text = prompt_user();
        println!("Writing msg (offline) to Register: '{input_text}'");
        let msg = format!("[{user}]: {input_text}");
        match reg_replica.write(msg.as_bytes()) {
            Ok(()) => {}
            Err(Error::ContentBranchDetected(branches)) => {
                println!(
                    "Branches ({}) detected in Register, let's merge them all...",
                    branches.len()
                );
                reg_replica.write_merging_branches(msg.as_bytes())?;
            }
            Err(err) => return Err(err.into()),
        }

        // Sync with network after a delay
        println!("Syncing with SAFE in {delay:?}...");
        sleep(delay).await;
        println!("synced!");

        reg_replica.sync().await?;
    }
}

fn prompt_user() -> String {
    let mut input_text = String::new();
    println!();
    println!("Enter new text to write onto the Register:");
    io::stdin()
        .read_line(&mut input_text)
        .expect("Failed to read text from stdin");

    input_text.trim().to_string()
}
