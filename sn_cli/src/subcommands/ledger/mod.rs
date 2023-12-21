// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod get_app_info;
mod get_pub_key;
mod sign_tx;

use get_app_info::GetAppInfoReq;
use get_pub_key::GetPubKeyReq;
use sign_tx::SignTxReq;

use sn_transfers::{Hash, InputLedger, NanoTokens, OutputLedger, SpendLedger, TransactionLedger};

use bls::SecretKey;
use color_eyre::Result;
use ledger_lib::{
    transport::{TcpInfo, TcpTransport},
    Transport,
};

const MAX_REQ_SIZE: usize = 256;

const APDU_CLA: u8 = 0xe0;

struct ApduINS;
impl ApduINS {
    const GET_APP_INFO: u8 = 0x04;
    const GET_PUBLIC_KEY: u8 = 0x05;
    const SIGN_TX: u8 = 0x06;
}

struct ApduP1;
impl ApduP1 {
    const P1_START: u8 = 0x00;
    const P1_CONFIRM: u8 = 0x01;
}

struct ApduP2;
impl ApduP2 {
    const P2_LAST: u8 = 0x00;
    const P2_MORE: u8 = 0x80;
}

// Encode public key derivation path
fn derivation_path(pk_path: &[u8]) -> Vec<u8> {
    let mut dpath = vec![pk_path.len() as u8];
    for c in pk_path {
        dpath.push(0u8);
        dpath.push(0u8);
        dpath.push(0u8);
        dpath.push(*c);
    }
    dpath
}

pub(super) async fn ledger_get_addr() -> Result<()> {
    println!("** Using Ledger hardware wallet **");

    let mut device = TcpTransport::new()?
        .connect(TcpInfo::default()) // default socket is localhost::1237
        .await?;

    let key_path = &[44, 0, 0, 1];
    let apdu_get_public_key = GetPubKeyReq::new(key_path);
    let resp = apdu_get_public_key.send(&mut device).await?;
    println!("APDU get pub key response: {resp:?}");

    let apdu_get_app_info = GetAppInfoReq::default();
    let resp = apdu_get_app_info.send(&mut device).await?;
    println!("APDU get app info response: {resp:?}");

    let input = InputLedger {
        unique_pubkey: [0; 48],
        amount: NanoTokens::from(200),
    };
    let output = OutputLedger {
        unique_pubkey: [3; 48],
        amount: NanoTokens::from(100),
    };
    let tx = TransactionLedger {
        inputs: [input.clone()],
        outputs: [output.clone()],
    };

    let spend = SpendLedger {
        unique_pubkey: SecretKey::random().public_key().to_bytes(),
        spent_tx: tx.clone(),
        reason: Hash::default(),
        token: NanoTokens::from(15),
        parent_tx: tx,
    };

    let mut apdu_sign_tx = SignTxReq::new(key_path, &spend);
    let resp = apdu_sign_tx.send(&mut device).await;
    println!("APDU sign spend response: {resp:?}");
    resp?;
    Ok(())
}
