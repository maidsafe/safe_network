// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::send_tokens;
#[cfg(feature = "distribution")]
use base64::Engine;
use color_eyre::eyre::{eyre, Result};
use serde::{Deserialize, Serialize};
use sn_client::Client;
use sn_transfers::{MainPubkey, NanoTokens};
use std::str::FromStr;
use std::{collections::HashMap, path::PathBuf};
use tracing::info;

const SNAPSHOT_FILENAME: &str = "snapshot.json";
const SNAPSHOT_URL: &str = "https://api.omniexplorer.info/ask.aspx?api=getpropertybalances&prop=3";
const CLAIMS_URL: &str =
    "https://github.com/maidsafe/safe_network/raw/main/sn_faucet/maid_address_claims.csv";
const HTTP_STATUS_OK: i32 = 200;

type MaidAddress = String; // base58 encoded
type Snapshot = HashMap<MaidAddress, NanoTokens>;

// Parsed from json in SNAPSHOT_URL
#[derive(Serialize, Deserialize)]
struct MaidBalance {
    address: MaidAddress,
    balance: String,
    reserved: String,
}

// Maid owners supply info that allows the faucet to distribute their funds.
// They sign a safe wallet address using their maid key to prove ownership of
// the maid.
// The faucet will distribute SNT directly to that safe wallet address.
pub struct MaidClaim {
    address: String,   // base58 encoded bitcoin address owning omni maid
    pubkey: String,    // hex encoded bitcoin public key
    wallet: String,    // hex encoded safe wallet address
    signature: String, // base64 encoded bitcoin signature of the wallet hex
}

impl MaidClaim {
    pub fn new(address: MaidAddress, wallet: String, signature: String) -> Result<MaidClaim> {
        let pubkey = match pubkey_from_signature(&wallet, &signature) {
            Ok(pk) => pk,
            Err(err) => {
                return Err(eyre!("Invalid signature: {err}"));
            }
        };
        let pubkey_hex = hex::encode(pubkey.to_bytes());
        let mc = MaidClaim {
            address,
            pubkey: pubkey_hex,
            wallet,
            signature,
        };
        mc.is_valid()?;
        Ok(mc)
    }

    pub fn from_csv_line(line: &str) -> Result<MaidClaim> {
        let cells = line.trim().split(',').collect::<Vec<&str>>();
        if cells.len() != 4 {
            let msg = format!("Invalid claim csv: {line}");
            return Err(eyre!(msg.to_string()));
        }
        let mc = MaidClaim {
            address: cells[0].to_string(),
            pubkey: cells[1].to_string(),
            wallet: cells[2].to_string(),
            signature: cells[3].to_string(),
        };
        mc.is_valid()?;
        Ok(mc)
    }

    pub fn to_csv_line(&self) -> String {
        format!(
            "{},{},{},{}",
            self.address, self.pubkey, self.wallet, self.signature
        )
    }

    pub fn is_valid(&self) -> Result<()> {
        // check signature is correct
        check_signature(&self.address, &self.wallet, &self.signature)?;
        // check pk matches address
        if !maid_pk_matches_address(&self.address, &self.pubkey) {
            return Err(eyre!("Claim public key does not match address"));
        }
        // check wallet is a valid bls pubkey
        if MainPubkey::from_hex(&self.wallet).is_err() {
            return Err(eyre!("Invalid bls public key"));
        };
        // if all the checks are ok, it's valid
        Ok(())
    }

    pub fn save_to_file(&self) -> Result<()> {
        // check it's valid before we write it, can't know for sure it was
        // already validated
        self.is_valid()?;
        // if it already exists, overwrite it
        let addr_path = get_claims_data_dir_path()?.join(self.address.clone());
        let csv_line = self.to_csv_line();
        std::fs::write(addr_path, csv_line)?;
        Ok(())
    }
}

// This is different to test_faucet_data_dir because it should *not* be
// removed when --clean flag is specified.
fn get_snapshot_data_dir_path() -> Result<PathBuf> {
    let dir = dirs_next::data_dir()
        .ok_or_else(|| eyre!("could not obtain data directory path".to_string()))?
        .join("safe_snapshot");
    std::fs::create_dir_all(dir.clone())?;
    Ok(dir.to_path_buf())
}

fn get_claims_data_dir_path() -> Result<PathBuf> {
    let dir = dirs_next::data_dir()
        .ok_or_else(|| eyre!("could not obtain data directory path".to_string()))?
        .join("safe_snapshot")
        .join("claims");
    std::fs::create_dir_all(dir.clone())?;
    Ok(dir.to_path_buf())
}

fn get_distributions_data_dir_path() -> Result<PathBuf> {
    let dir = dirs_next::data_dir()
        .ok_or_else(|| eyre!("could not obtain data directory path".to_string()))?
        .join("safe_snapshot")
        .join("distributions");
    std::fs::create_dir_all(dir.clone())?;
    Ok(dir.to_path_buf())
}

pub fn load_maid_snapshot() -> Result<Snapshot> {
    // If the faucet restarts there will be an existing snapshot which should
    // be used to avoid conflicts in the balances between two different
    // snapshots.
    // Check if a previous snapshot already exists
    let root_dir = get_snapshot_data_dir_path()?;
    let filename = root_dir.join(SNAPSHOT_FILENAME);
    if std::fs::metadata(filename.clone()).is_ok() {
        info!("Using existing maid snapshot from {:?}", filename);
        maid_snapshot_from_file(filename)
    } else {
        info!("Fetching snapshot from {}", SNAPSHOT_URL);
        maid_snapshot_from_internet(filename)
    }
}

fn maid_snapshot_from_file(snapshot_path: PathBuf) -> Result<Snapshot> {
    let content = std::fs::read_to_string(snapshot_path)?;
    parse_snapshot(content)
}

fn maid_snapshot_from_internet(snapshot_path: PathBuf) -> Result<Snapshot> {
    // make the request
    let response = minreq::get(SNAPSHOT_URL).send()?;
    // check the request is ok
    if response.status_code != HTTP_STATUS_OK {
        let msg = format!("Snapshot failed with http status {}", response.status_code);
        return Err(eyre!(msg));
    }
    // write the response to file
    let body = response.as_str()?;
    info!("Writing snapshot to {:?}", snapshot_path);
    std::fs::write(snapshot_path.clone(), body)?;
    info!("Saved snapshot to {:?}", snapshot_path);
    // parse the json response
    parse_snapshot(body.to_string())
}

fn parse_snapshot(json_str: String) -> Result<Snapshot> {
    let balances: Vec<MaidBalance> = serde_json::from_str(&json_str)?;
    let mut balances_map: Snapshot = Snapshot::new();
    // verify the snapshot is ok
    // balances must match the ico amount, which is slightly higher than
    // 2^32/10 because of the ico process.
    // see https://omniexplorer.info/asset/3
    let supply = NanoTokens::from(452_552_412_000_000_000);
    let mut total = NanoTokens::zero();
    for b in &balances {
        // The reserved amount is the amount currently for sale on omni dex.
        // If it's not included the total is lower than expected.
        // So the amount of maid an address owns is balance + reserved.
        let balance = NanoTokens::from_str(&b.balance)?;
        let reserved = NanoTokens::from_str(&b.reserved)?;
        let address_balance = match balance.checked_add(reserved) {
            Some(b) => b,
            None => {
                let msg = format!("Nanos overflowed adding maid {balance} + {reserved}");
                return Err(eyre!(msg));
            }
        };
        total = match total.checked_add(address_balance) {
            Some(b) => b,
            None => {
                let msg = format!("Nanos overflowed adding maid {total} + {address_balance}");
                return Err(eyre!(msg));
            }
        };
        balances_map.insert(b.address.clone(), address_balance);
    }
    if total != supply {
        let msg = format!("Incorrect snapshot total, got {total} want {supply}");
        return Err(eyre!(msg));
    }
    // log the total number of balances that were parsed
    info!("Parsed {} maid balances from the snapshot", balances.len());
    Ok(balances_map)
}

fn load_maid_claims_from_local() -> Result<HashMap<MaidAddress, MaidClaim>> {
    let mut claims = HashMap::new();
    // load from existing files
    let claims_dir = get_claims_data_dir_path()?;
    let file_list = std::fs::read_dir(claims_dir)?;
    for file in file_list {
        // add to hashmap
        let file = file?;
        let claim_csv = std::fs::read_to_string(file.path())?;
        let claim = MaidClaim::from_csv_line(&claim_csv)?;
        claims.insert(claim.address.clone(), claim);
    }
    Ok(claims)
}

pub fn load_maid_claims() -> Result<HashMap<MaidAddress, MaidClaim>> {
    info!("Loading claims for distributions");
    let mut claims = match load_maid_claims_from_local() {
        Ok(claims) => claims,
        Err(err) => {
            info!("Failed to load claims from local, {err:?}");
            HashMap::new()
        }
    };
    info!("{} claims after reading existing files", claims.len());

    // load from list on internet
    info!("Fetching claims from {CLAIMS_URL}");
    let response = minreq::get(CLAIMS_URL).send()?;
    // check the request is ok
    if response.status_code != 200 {
        println!(
            "Claims request failed with http status {}",
            response.status_code
        );
        // The existing data is ok, no need to fail to start the server here
        return Ok(claims);
    }
    // parse the response as csv, each row has format:
    // address,pkhex,wallet,signature
    let body = response.as_str()?;
    let lines: Vec<&str> = body.trim().split('\n').collect();
    info!("{} claims rows from {CLAIMS_URL}", lines.len());
    for line in lines {
        let claim = match MaidClaim::from_csv_line(line) {
            Ok(c) => c,
            Err(_) => {
                continue;
            }
        };
        // validate this claim info all matches correctly
        if claim.is_valid().is_err() {
            continue;
        }
        // save this cliam to the file system
        if claim.save_to_file().is_err() {
            println!("Error saving claim to file");
            continue;
        }
        // add this claim to the hashmap
        claims.insert(claim.address.clone(), claim);
    }
    info!("{} claims after reading from online list", claims.len());
    Ok(claims)
}

fn maid_pk_matches_address(address: &str, pk_hex: &str) -> bool {
    // parse the address
    let addr = match bitcoin::Address::from_str(address) {
        Ok(a) => a,
        Err(_) => return false,
    };
    let btc_addr = match addr.clone().require_network(bitcoin::Network::Bitcoin) {
        Ok(a) => a,
        Err(_) => return false,
    };
    // parse the public key
    let pk = match bitcoin::PublicKey::from_str(pk_hex) {
        Ok(p) => p,
        Err(_) => return false,
    };
    // The public key may be for a p2pkh address (starting with 1) or a p2wpkh
    // address (starting with 3) so we need to check both.
    let is_p2pkh = btc_addr.is_related_to_pubkey(&pk);
    if is_p2pkh {
        return true;
    }
    let p2wpkh_addr = match bitcoin::Address::p2shwpkh(&pk, bitcoin::Network::Bitcoin) {
        Ok(a) => a,
        Err(_) => return false,
    };
    let is_p2wpkh = p2wpkh_addr == addr;
    if is_p2wpkh {
        return true;
    }
    false
}

fn check_signature(address: &MaidAddress, msg: &str, signature: &str) -> Result<()> {
    let secp = bitcoin::secp256k1::Secp256k1::new(); // DevSkim: ignore DS440100
    let msg_hash = bitcoin::sign_message::signed_msg_hash(msg);
    let sig = bitcoin::sign_message::MessageSignature::from_str(signature)?;
    // Signatures doesn't work with p2wpkh-p2sh so always use p2pkh addr.
    // This was double checked with electrum signature validation.
    let mut addr =
        bitcoin::Address::from_str(address)?.require_network(bitcoin::Network::Bitcoin)?;
    let pubkey = pubkey_from_signature(msg, signature)?;
    if address.starts_with('3') {
        addr = bitcoin::Address::p2pkh(&pubkey, bitcoin::Network::Bitcoin);
    }
    // check the signature is correct
    if !sig.is_signed_by_address(&secp, &addr, msg_hash)? {
        return Err(eyre!("Invalid signature"));
    }
    // Check the pubkey in the signature matches the address.
    // This prevents someone submitting a valid signature from a pubkey that
    // doesn't match the address for the snapshot.
    let pubkey_hex = hex::encode(pubkey.to_bytes());
    if !maid_pk_matches_address(address, &pubkey_hex) {
        return Err(eyre!("Public key does not match address"));
    }
    Ok(())
}

fn pubkey_from_signature(msg: &str, signature: &str) -> Result<bitcoin::PublicKey> {
    let secp = bitcoin::secp256k1::Secp256k1::new(); // DevSkim: ignore DS440100
    let msg_hash = bitcoin::sign_message::signed_msg_hash(msg);
    let sig = match bitcoin::sign_message::MessageSignature::from_base64(signature) {
        Ok(s) => s,
        Err(err) => {
            let msg = format!("Error parsing signature: {err}");
            return Err(eyre!(msg));
        }
    };
    let pubkey = sig.recover_pubkey(&secp, msg_hash)?;
    Ok(pubkey)
}

pub async fn distribute_from_maid_to_tokens(
    client: Client,
    snapshot: Snapshot,
    claims: HashMap<MaidAddress, MaidClaim>,
) {
    for (addr, amount) in snapshot {
        // check if this snapshot address has a pubkey
        if !claims.contains_key(&addr) {
            continue;
        }
        let claim = &claims[&addr];
        match create_distribution(&client, claim, &amount).await {
            Ok(_) => {}
            Err(err) => {
                info!(
                    "Error creating distribution: {0} {err}",
                    claim.to_csv_line()
                );
            }
        }
    }
}

pub async fn handle_distribution_req(
    client: &Client,
    query: HashMap<String, String>,
    balances: Snapshot,
) -> Result<String> {
    let address = query
        .get("address")
        .ok_or(eyre!("Missing address in querystring"))?
        .to_string();
    let wallet = query
        .get("wallet")
        .ok_or(eyre!("Missing wallet in querystring"))?
        .to_string();
    let signature = query
        .get("signature")
        .ok_or(eyre!("Missing signature in querystring"))?
        .to_string();
    let amount = balances
        .get(&address)
        .ok_or(eyre!("Address not in snapshot"))?;
    // Bitcoin expects base64 standard encoding but the query string has
    // base64 url encoding, so the sig is converted to standard encoding
    let sig_bytes = base64::engine::general_purpose::URL_SAFE.decode(signature)?;
    let sig = base64::engine::general_purpose::STANDARD.encode(sig_bytes);
    let claim = MaidClaim::new(address, wallet, sig)?;
    create_distribution(client, &claim, amount).await
}

async fn create_distribution(
    client: &Client,
    claim: &MaidClaim,
    amount: &NanoTokens,
) -> Result<String> {
    // validate the claim
    if claim.is_valid().is_err() {
        let claim_csv = claim.to_csv_line();
        let msg = format!("Not creating distribution for invalid claim: {claim_csv}");
        info!(msg);
        return Err(eyre!(msg));
    }
    // save this claim to file
    claim.save_to_file()?;
    // check if this distribution has already been created
    let root = get_distributions_data_dir_path()?;
    let dist_path = root.join(&claim.address);
    if dist_path.exists() {
        let dist_hex = match std::fs::read_to_string(dist_path.clone()) {
            Ok(content) => content,
            Err(err) => {
                let msg = format!(
                    "Error reading distribution file {}: {}",
                    dist_path.display(),
                    err
                );
                info!(msg);
                return Err(eyre!(msg));
            }
        };
        return Ok(dist_hex);
    }
    info!(
        "Distributing {} for {} to {}",
        amount, claim.address, claim.wallet
    );
    // create a transfer to the claim wallet
    let transfer_hex = match send_tokens(client, &amount.to_string(), &claim.wallet).await {
        Ok(t) => t,
        Err(err) => {
            let msg = format!("Failed send for {0}: {err}", claim.address);
            info!(msg);
            return Err(eyre!(msg));
        }
    };
    let _ = match hex::decode(transfer_hex.clone()) {
        Ok(t) => t,
        Err(err) => {
            let msg = format!("Failed to decode transfer for {0}: {err}", claim.address);
            info!(msg);
            return Err(eyre!(msg));
        }
    };
    // save the transfer
    match std::fs::write(dist_path.clone(), transfer_hex.clone()) {
        Ok(_) => {}
        Err(err) => {
            let msg = format!(
                "Failed to write transfer to file {}: {}",
                dist_path.display(),
                err
            );
            info!(msg);
            info!("The transfer hex that failed to write to file:");
            info!(transfer_hex);
            return Err(eyre!(msg));
        }
    };
    Ok(transfer_hex)
}

#[cfg(all(test, feature = "distribution"))]
mod tests {
    use super::*;

    use assert_fs::TempDir;
    use bitcoin::{
        hashes::Hash,
        secp256k1::{rand, Secp256k1},
        Address, Network, PublicKey,
    };
    use sn_logging::LogBuilder;
    use sn_transfers::{HotWallet, MainSecretKey, Transfer};

    // This test is to confirm fetching 'MAID snapshop` and `Maid claims` list from website
    // is working properly and giving consistent and expected result.
    //
    // Note: the current list will grow as testnets collect more claims
    #[test]
    fn fetching_from_network() -> Result<()> {
        let snapshot = load_maid_snapshot()?;
        println!("Maid snapshot got {:?} entries", snapshot.len());
        assert!(!snapshot.is_empty());

        let claims = load_maid_claims()?;
        println!("Got {:?} distribution claims", claims.len());

        Ok(())
    }

    // This test will simulate a token distribution.
    #[tokio::test]
    async fn token_distribute_to_user() -> Result<()> {
        let _log_guards =
            LogBuilder::init_single_threaded_tokio_test("token_distribute_to_user test");

        let amount = NanoTokens::from(10);

        let secp = Secp256k1::new(); // DevSkim: ignore DS440100
        let (maid_secret_key, maid_public_key) = secp.generate_keypair(&mut rand::thread_rng());
        let maid_address = Address::p2pkh(&PublicKey::new(maid_public_key), Network::Bitcoin);

        let client_token_issuer = Client::quick_start(None).await?;

        // wallet comes from `safe wallet address`
        let wallet_sk = bls::SecretKey::random();
        let wallet_pk_hex = wallet_sk.public_key().to_hex();
        // signature comes from bitcoin signing like electrum or trezor
        let msg_hash = bitcoin::sign_message::signed_msg_hash(&wallet_pk_hex);
        let msg = bitcoin::secp256k1::Message::from_digest(msg_hash.to_byte_array()); // DevSkim: ignore DS440100
        let secp_sig = secp.sign_ecdsa_recoverable(&msg, &maid_secret_key);
        let signature = bitcoin::sign_message::MessageSignature {
            signature: secp_sig,
            compressed: true,
        };
        let claim = MaidClaim::new(
            maid_address.to_string(),
            wallet_pk_hex,
            signature.to_string(),
        )?;

        let transfer_hex = create_distribution(&client_token_issuer, &claim, &amount).await?;

        let transfer = Transfer::from_hex(&transfer_hex)?;

        assert!(transfer
            .cashnote_redemptions(&MainSecretKey::new(wallet_sk.clone()))
            .is_ok());

        let receiver_client = Client::new(bls::SecretKey::random(), None, None, None).await?;
        let tmp_path = TempDir::new()?.path().to_owned();
        let receiver_wallet =
            HotWallet::load_from_path(&tmp_path, Some(MainSecretKey::new(wallet_sk)))?;

        let mut cash_notes = receiver_client.receive(&transfer, &receiver_wallet).await?;
        assert_eq!(cash_notes.len(), 1);
        let cash_note = cash_notes.pop().unwrap();

        assert_eq!(cash_note.value()?, amount);

        Ok(())
    }

    #[test]
    fn maidclaim_isvalid() -> Result<()> {
        // Signatures generated using electrum to ensure interoperability.

        // prvkey for addr 17ig7... is L4DDUabuAU9AxVepwNkLBDmvrG4TXLJFDHoKPtkJdyDAPM3zHQhu
        // sig is valid for wallet_a signed by addr_a
        const MAID_ADDR_A: &str = "17ig7FYbSDaZZqVEjFmrGv7GSXBNLeJPNG";
        const MAID_PUBKEY_A: &str =
            "0383f4c6f1a3624140ba587e4ea5c6264a94d4077c1cf4ca7714bb93c67b3262bc"; // DevSkim: ignore DS173237
        const WALLET_A: &str = "ac1e81dd3ccb28d4e7d8e551e953279d8af1ede5bbdbbb71aefb78a43206ca7827a3279160da4ee8c7296dfac72f8c8a"; // DevSkim: ignore DS173237
        const SIG_A: &str = "HxaGOcmLu1BrSwzBi+KazC6XHbX/6B1Eyf9CnJrxB/OeKdJP9Jp38s+eqfBZ73wLG1OJW0mURhAmZkCsvBJayPM=";

        // prvkey for addr 1EbjF... is L2gzGZUqifkBG3jwwkyyfos8A67VvFhyrtqKU5cWkfEpySkFbaBR
        // sig is valid for wallet_b signed by addr_b
        const MAID_PUBKEY_B: &str =
            "031bc89b9279ae36795910c0d173002504f2c22dd45368263a5f30ce68e8696e0f"; // DevSkim: ignore DS173237
        const WALLET_B: &str = "915d803d302bc1270e20de34413c270bdc4be632880e577719c2bf7d22e2c7b44388feef17fe5ac86b5d561697f2b3bf"; // DevSkim: ignore DS173237
        const SIG_B: &str = "Hy3zUK3YiEidzE+HpdgeoRoH3lkCrOoTh59TvoOiUdfJVKKLAVUuAydgIJkOTVU8JKdvbYPGiQhf7KCiNtLRIVU=";

        // not a valid bls wallet (starting with 0)
        // sig is valid for wallet_c signed by addr_a
        const WALLET_C: &str = "015d803d302bc1270e20de34413c270bdc4be632880e577719c2bf7d22e2c7b44388feef17fe5ac86b5d561697f2b3bf"; // DevSkim: ignore DS173237
        const SIG_C: &str = "IE8y8KSRKw3hz/rd9dzrJLOu24sAspuJgYr6VVGCga3FQQhzOEFDKZoDdrJORRI4Rvv7vFqRARQVaBKCobYh9sc=";

        // MaidClaim::new calls is_valid
        let mc = MaidClaim::new(
            MAID_ADDR_A.to_string(),
            WALLET_A.to_string(),
            SIG_A.to_string(),
        );
        assert!(mc.is_ok());

        // MaidClaim::new will fail if inputs are incorrect
        // because new calls is_valid
        let mc = MaidClaim::new(
            MAID_ADDR_A.to_string(),
            WALLET_A.to_string(),
            SIG_B.to_string(),
        );
        assert!(mc.is_err());

        // valid
        let mc = MaidClaim {
            address: MAID_ADDR_A.to_string(),
            pubkey: MAID_PUBKEY_A.to_string(),
            wallet: WALLET_A.to_string(),
            signature: SIG_A.to_string(),
        };
        assert!(mc.is_valid().is_ok());

        // pk not matching address
        let mc = MaidClaim {
            address: MAID_ADDR_A.to_string(),
            pubkey: MAID_PUBKEY_B.to_string(),
            wallet: WALLET_A.to_string(),
            signature: SIG_A.to_string(),
        };
        assert!(mc.is_valid().is_err());

        // signature not matching message
        let mc = MaidClaim {
            address: MAID_ADDR_A.to_string(),
            pubkey: MAID_PUBKEY_A.to_string(),
            wallet: WALLET_A.to_string(),
            signature: SIG_B.to_string(),
        };
        assert!(mc.is_valid().is_err());

        // signature matches message but not address
        let mc = MaidClaim {
            address: MAID_ADDR_A.to_string(),
            pubkey: MAID_PUBKEY_B.to_string(),
            wallet: WALLET_B.to_string(),
            signature: SIG_B.to_string(),
        };
        assert!(mc.is_valid().is_err());

        // wallet is not a valid bls key
        let mc = MaidClaim {
            address: MAID_ADDR_A.to_string(),
            pubkey: MAID_PUBKEY_A.to_string(),
            wallet: WALLET_C.to_string(),
            signature: SIG_C.to_string(),
        };
        assert!(mc.is_valid().is_err());

        Ok(())
    }

    #[test]
    fn pk_matches_addr() -> Result<()> {
        // p2pkh compressed
        assert!(maid_pk_matches_address(
            "17ig7FYbSDaZZqVEjFmrGv7GSXBNLeJPNG",
            "0383f4c6f1a3624140ba587e4ea5c6264a94d4077c1cf4ca7714bb93c67b3262bc", // DevSkim: ignore DS173237
        ));

        // p2pkh uncompressed
        assert!(maid_pk_matches_address(
            "1QK8WWMcDEFUVV2zKU8GSCwwuvAFWEs2QW",
            "0483f4c6f1a3624140ba587e4ea5c6264a94d4077c1cf4ca7714bb93c67b3262bc4327efb5ba23543c8a6e63ddc09618e11b5d0d184bb69f964712d0894c005655", // DevSkim: ignore DS173237
        ));

        // p2wpkh-p2sh
        assert!(maid_pk_matches_address(
            "3GErA71Kz6Tn4QCLqoaDvMxD5cLgqQLykv",
            "03952005f63e148735d244dc52253586c6ed89d1692599452e7daaa2a63a88619a", // DevSkim: ignore DS173237
        ));

        // mismatched returns false
        assert!(!maid_pk_matches_address(
            "17ig7FYbSDaZZqVEjFmrGv7GSXBNLeJPNG",
            "031bc89b9279ae36795910c0d173002504f2c22dd45368263a5f30ce68e8696e0f", // DevSkim: ignore DS173237
        ));

        Ok(())
    }

    #[test]
    fn pubkey_from_sig() -> Result<()> {
        // Valid message and signature produces the corresponding public key.
        // Signatures generated using electrum to ensure interoperability

        // p2pkh compressed
        // electrum import key
        // L4DDUabuAU9AxVepwNkLBDmvrG4TXLJFDHoKPtkJdyDAPM3zHQhu
        let pubkey = pubkey_from_signature(
            "ac1e81dd3ccb28d4e7d8e551e953279d8af1ede5bbdbbb71aefb78a43206ca7827a3279160da4ee8c7296dfac72f8c8a", // DevSkim: ignore DS173237
            "HxaGOcmLu1BrSwzBi+KazC6XHbX/6B1Eyf9CnJrxB/OeKdJP9Jp38s+eqfBZ73wLG1OJW0mURhAmZkCsvBJayPM=",
        )?;
        let pubkey_hex = hex::encode(pubkey.to_bytes());
        assert_eq!(
            pubkey_hex,
            "0383f4c6f1a3624140ba587e4ea5c6264a94d4077c1cf4ca7714bb93c67b3262bc" // DevSkim: ignore DS173237
        );

        // p2pkh uncompressed
        // electrum import key
        // 5Jz2acAoqLr57YXzQuoiNS8sQtZQ3TBcVcaKsX5ybp9HtJiUSXq
        let pubkey = pubkey_from_signature(
            "ac1e81dd3ccb28d4e7d8e551e953279d8af1ede5bbdbbb71aefb78a43206ca7827a3279160da4ee8c7296dfac72f8c8a", // DevSkim: ignore DS173237
            "Gw2YmGq5cbXVOCZKd1Uwku/kn9UWJ8QYGlho+FTXokfeNbQzINKli73rvoi39ssVN825kn5LgSdNu800e3w+eXE=",
        )?;
        let pubkey_hex = hex::encode(pubkey.to_bytes());
        assert_eq!(
            pubkey_hex,
            "04952005f63e148735d244dc52253586c6ed89d1692599452e7daaa2a63a88619a0418114ad86aeda109dd924629bbf929e82c6ce5be948e4d21a95575a53e1f73" // DevSkim: ignore DS173237
        );

        // p2wpkh-p2sh uncompressed
        // electrum import key
        // p2wpkh-p2sh:L2NhyLEHiNbb9tBnQY5BbbwjWSZzhpZqfJ26Hynxpf5bXL9sUm73
        let pubkey = pubkey_from_signature(
            "ac1e81dd3ccb28d4e7d8e551e953279d8af1ede5bbdbbb71aefb78a43206ca7827a3279160da4ee8c7296dfac72f8c8a", // DevSkim: ignore DS173237
            "Hw2YmGq5cbXVOCZKd1Uwku/kn9UWJ8QYGlho+FTXokfeNbQzINKli73rvoi39ssVN825kn5LgSdNu800e3w+eXE=",
        )?;
        let pubkey_hex = hex::encode(pubkey.to_bytes());
        assert_eq!(
            pubkey_hex,
            "03952005f63e148735d244dc52253586c6ed89d1692599452e7daaa2a63a88619a" // DevSkim: ignore DS173237
        );

        Ok(())
    }
}
