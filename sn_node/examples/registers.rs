// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sn_client::{
    acc_packet::load_account_wallet_or_create_with_mnemonic, Client, Error, WalletClient,
};
use sn_registers::{Permissions, RegisterAddress};

use xor_name::XorName;

use bls::SecretKey;
use clap::Parser;
use color_eyre::{
    eyre::{eyre, Result, WrapErr},
    Help,
};
use std::{io, time::Duration};
use tokio::time::sleep;

#[derive(Parser, Debug)]
#[clap(name = "registers cli")]
struct Opt {
    // A name for this user in the example
    #[clap(long)]
    user: String,

    // Create register and give it a nickname (first user)
    #[clap(long, default_value = "")]
    reg_nickname: String,

    // Get existing register with given network address (any other user)
    #[clap(long, default_value = "", conflicts_with = "reg_nickname")]
    reg_address: String,

    // Delay before synchronising local register with the network
    #[clap(long, default_value_t = 2000)]
    delay_millis: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::parse();
    let user = opt.user;
    let mut reg_nickname = opt.reg_nickname;
    let reg_address_string = opt.reg_address;
    let delay = Duration::from_millis(opt.delay_millis);

    // let's build a random secret key to sign our Register ops
    let signer = SecretKey::random();

    println!("Starting SAFE client...");
    let client = Client::new(signer, None, None, None).await?;
    println!("SAFE client signer public key: {:?}", client.signer_pk());

    // We'll retrieve (or create if not found) a Register, and write on it
    // in offline mode, syncing with the network periodically.

    let mut meta = XorName::from_content(reg_nickname.as_bytes());
    let reg_address = if !reg_nickname.is_empty() {
        meta = XorName::from_content(reg_nickname.as_bytes());
        RegisterAddress::new(meta, client.signer_pk())
    } else {
        reg_nickname = format!("{reg_address_string:<6}...");
        RegisterAddress::from_hex(&reg_address_string)
            .wrap_err("cannot parse hex register address")?
    };

    // Loading a local wallet. It needs to have a non-zero balance for
    // this example to be able to pay for the Register's storage.
    let root_dir = dirs_next::data_dir()
        .ok_or_else(|| eyre!("could not obtain data directory path".to_string()))?
        .join("safe")
        .join("client");

    let wallet = load_account_wallet_or_create_with_mnemonic(&root_dir, None)
        .wrap_err("Unable to read wallet file in {root_dir:?}")
        .suggestion(
            "If you have an old wallet file, it may no longer be compatible. Try removing it",
        )?;
    let mut wallet_client = WalletClient::new(client.clone(), wallet);

    println!("Retrieving Register '{reg_nickname}' from SAFE, as user '{user}'");
    let mut reg_replica = match client.get_register(reg_address).await {
        Ok(register) => {
            println!(
                "Register '{reg_nickname}' found at {:?}!",
                register.address(),
            );
            register
        }
        Err(_) => {
            println!("Register '{reg_nickname}' not found, creating it at {reg_address}");
            let (register, _cost, _royalties_fees) = client
                .create_and_pay_for_register(
                    meta,
                    &mut wallet_client,
                    true,
                    Permissions::new_anyone_can_write(),
                )
                .await?;

            register
        }
    };
    println!("Register address: {:?}", reg_replica.address().to_hex());
    println!("Register owned by: {:?}", reg_replica.owner());
    println!("Register permissions: {:?}", reg_replica.permissions());

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
        if !input_text.is_empty() {
            println!("Writing msg (offline) to Register: '{input_text}'");
            let msg = format!("[{user}]: {input_text}");
            match reg_replica.write(msg.as_bytes()) {
                Ok(_) => {}
                Err(Error::ContentBranchDetected(branches)) => {
                    println!(
                        "Branches ({}) detected in Register, let's merge them all...",
                        branches.len()
                    );
                    reg_replica.write_merging_branches(msg.as_bytes())?;
                }
                Err(err) => return Err(err.into()),
            }
        }

        // Sync with network after a delay
        println!("Syncing with SAFE in {delay:?}...");
        sleep(delay).await;
        reg_replica.sync(&mut wallet_client, true, None).await?;
        println!("synced!");
    }
}

fn prompt_user() -> String {
    let mut input_text = String::new();
    println!();
    println!("Enter a blank line to receive updates, or some text to be written.");
    io::stdin()
        .read_line(&mut input_text)
        .expect("Failed to read text from stdin");

    input_text.trim().to_string()
}
