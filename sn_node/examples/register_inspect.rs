// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crdts::merkle_reg::{Hash, MerkleReg, Node};
use std::collections::HashMap;
use std::io;

use sn_client::{acc_packet::load_account_wallet_or_create_with_mnemonic, Client, WalletClient};
use sn_registers::{Entry, Permissions, RegisterAddress};

use xor_name::XorName;

use bls::SecretKey;
use clap::Parser;
use color_eyre::{
    eyre::{eyre, Result, WrapErr},
    Help,
};

#[derive(Parser, Debug)]
#[clap(name = "register inspect cli")]
struct Opt {
    // Create register and give it a nickname (first user)
    #[clap(long, default_value = "")]
    reg_nickname: String,

    // Get existing register with given network address (any other user)
    #[clap(long, default_value = "", conflicts_with = "reg_nickname")]
    reg_address: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::parse();
    let mut reg_nickname = opt.reg_nickname;
    let reg_address_string = opt.reg_address;

    // let's build a random secret key to sign our Register ops
    let signer = SecretKey::random();

    println!("Starting SAFE client...");
    let client = Client::new(signer, None, None, None).await?;
    println!("SAFE client signer public key: {:?}", client.signer_pk());

    // The address of the register to be displayed
    let mut meta = XorName::from_content(reg_nickname.as_bytes());
    let reg_address = if !reg_nickname.is_empty() {
        meta = XorName::from_content(reg_nickname.as_bytes());
        RegisterAddress::new(meta, client.signer_pk())
    } else {
        reg_nickname = format!("{reg_address_string:<6}...");
        RegisterAddress::from_hex(&reg_address_string)
            .wrap_err("cannot parse hex register address")?
    };

    // Loading a local wallet (for ClientRegister::sync()).
    // The wallet can have ZERO balance in this example,
    // but the ClientRegister::sync() API requires a wallet and will
    // create the register if not found even though we don't want that.
    //
    // The only want to avoid unwanted creation of a Register seems to
    // be to supply an empty wallet.
    // TODO Follow the issue about this: https://github.com/maidsafe/safe_network/issues/1308
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

    println!("Retrieving Register '{reg_nickname}' from SAFE");
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

    // Repeatedly display of the register structure on command
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

        if prompt_user() {
            return Ok(());
        }

        // Sync with network after a delay
        println!("Syncing with SAFE...");
        reg_replica.sync(&mut wallet_client, true, None).await?;
        let merkle_reg = reg_replica.merkle_reg();
        let content = merkle_reg.read();
        println!("synced!");

        // Show the Register structure

        // Index nodes to make it easier to see where a
        // node appears multiple times in the output.
        // Note: it isn't related to the order of insertion
        // which is hard to determine.
        let mut index: usize = 0;
        let mut node_ordering: HashMap<Hash, usize> = HashMap::new();
        for (_hash, node) in content.hashes_and_nodes() {
            index_node_and_descendants(node, &mut index, &mut node_ordering, merkle_reg);
        }

        println!("======================");
        println!("Root (Latest) Node(s):");
        for node in content.nodes() {
            let _ = print_node(0, node, &node_ordering);
        }

        println!("======================");
        println!("Register Structure:");
        println!("(In general, earlier nodes are more indented)");
        let mut indents = 0;
        for (_hash, node) in content.hashes_and_nodes() {
            print_node_and_descendants(&mut indents, node, &node_ordering, merkle_reg);
        }

        println!("======================");
    }
}

fn index_node_and_descendants(
    node: &Node<Entry>,
    index: &mut usize,
    node_ordering: &mut HashMap<Hash, usize>,
    merkle_reg: &MerkleReg<Entry>,
) {
    let node_hash = node.hash();
    if node_ordering.get(&node_hash).is_none() {
        node_ordering.insert(node_hash, *index);
        *index += 1;
    }

    for child_hash in node.children.iter() {
        if let Some(child_node) = merkle_reg.node(*child_hash) {
            index_node_and_descendants(child_node, index, node_ordering, merkle_reg);
        } else {
            println!("ERROR looking up hash of child");
        }
    }
}

fn print_node_and_descendants(
    indents: &mut usize,
    node: &Node<Entry>,
    node_ordering: &HashMap<Hash, usize>,
    merkle_reg: &MerkleReg<Entry>,
) {
    let _ = print_node(*indents, node, node_ordering);

    *indents += 1;
    for child_hash in node.children.iter() {
        if let Some(child_node) = merkle_reg.node(*child_hash) {
            print_node_and_descendants(indents, child_node, node_ordering, merkle_reg);
        }
    }
    *indents -= 1;
}

fn print_node(
    indents: usize,
    node: &Node<Entry>,
    node_ordering: &HashMap<Hash, usize>,
) -> Result<()> {
    let order = match node_ordering.get(&node.hash()) {
        Some(order) => format!("{order}"),
        None => String::new(),
    };
    let indentation = "  ".repeat(indents);
    println!(
        "{indentation}[{:>2}] Node({:?}..) Entry({:?})",
        order,
        hex::encode(&node.hash()[0..3]),
        String::from_utf8(node.value.clone())?
    );
    Ok(())
}

fn prompt_user() -> bool {
    let mut input_text = String::new();
    println!();
    println!("Enter a blank line to print the latest register structure (or 'Q' <Enter> to quit)");
    io::stdin()
        .read_line(&mut input_text)
        .expect("Failed to read text from stdin");

    let string = input_text.trim().to_string();

    string.contains('Q') || string.contains('q')
}
