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
use color_eyre::{eyre::bail, Result};
use ledger_lib::{
    info::ConnInfo,
    transport::{BleTransport, GenericDevice, TcpInfo, TcpTransport, UsbTransport},
    Filters, LedgerProvider, Transport,
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
fn derivation_path(pk_path: &[u32]) -> Vec<u8> {
    let mut dpath = vec![pk_path.len() as u8];
    for item in pk_path {
        dpath.extend(item.to_be_bytes());
    }
    dpath
}

pub(super) async fn ledger_get_addr() -> Result<()> {
    println!("** Using Ledger hardware wallet **");

    let mut ledger_provider = LedgerProvider::init().await;
    let mut device = match ledger_provider.list(Filters::Any).await {
        Ok(devices) if devices.is_empty() => {
            println!("No USB devices detected automatically, triying with TCP on localhost...");
            let tcp_device = TcpTransport::new()?
                .connect(TcpInfo::default()) // default socket is localhost::1237
                .await?;
            GenericDevice::from(tcp_device)
        }
        Err(err) => {
            println!("Error when trying to detect devices through USB: {err:?}");
            println!("No devices detected through USB, triying with TCP on localhost...");
            let tcp_device = TcpTransport::new()?
                .connect(TcpInfo::default()) // default socket is localhost::1237
                .await?;
            GenericDevice::from(tcp_device)
        }
        Ok(devices) => {
            println!("Devices detected: {devices:?}");
            // let's just choose the first one to connect to
            match &devices[0].conn {
                ConnInfo::Usb(usb_info) => {
                    println!("Connecting to Ledger through USB: {usb_info:?}");
                    let mut usb_transport = UsbTransport::new()?;
                    let usb_device = usb_transport.connect(usb_info.clone()).await?;
                    GenericDevice::from(usb_device)
                }
                ConnInfo::Tcp(tcp_info) => {
                    println!("Connecting to Ledger through TCP: {tcp_info:?}");
                    let tcp_device = TcpTransport::new()?.connect(tcp_info.clone()).await?;
                    GenericDevice::from(tcp_device)
                }
                ConnInfo::Ble(ble_info) => {
                    println!("Connecting to Ledger through BLE: {ble_info:?}");
                    let ble_device = BleTransport::new().await?.connect(ble_info.clone()).await?;
                    GenericDevice::from(ble_device)
                }
            }
        }
    };

    println!("Connected to device: {}", device.info());

    // Key derivation path: m/12381/3600/0/0/0
    // Derived pk: ae7c293650c598098d67058e68752da5e534b8a0dcef2836976f033fdc4492c702e8afed2679ea4d3d5172617f95b0ee
    // Derived sk: 4404d69c70700aa3b37ca1529369f933687bffaf55795245672cc5e2b18d5357
    let key_path_0 = &[12381, 3600, 0, 0, 0];

    // Key derivation path: m/12381/3600/1/0/0
    // Derived pk: 9450d9c25fa466ed4334bc39103b7cd521d36562aefc5f475dcb602f15890565461f2d215f7cf552f3b213d02a84f375
    // Derived sk: 0eaf02a1872d1aec0d7bfc239c6b1cc38c4f4cbf8be8a3c64145a36073ce9889
    let key_path_1 = &[12381, 3600, 1, 0, 0];

    /*
    let apdu_get_public_key = GetPubKeyReq::new(key_path_0);
    let apdu_response = apdu_get_public_key.send(&mut device).await?;
    println!("APDU get pub key response: {apdu_response:?}");

    let length = apdu_response.data[0];
    println!("Public Key length: {length}");

    if length as usize != bls::PK_SIZE || apdu_response.data.len() < bls::PK_SIZE + 1 {
        bail!(
            "The response data/pk length ({}) doesn't match expected BLS pk length ({})",
            length,
            bls::PK_SIZE
        );
    }

    let mut pk_bytes = [0u8; bls::PK_SIZE];
    pk_bytes.copy_from_slice(&apdu_response.data[1..bls::PK_SIZE + 1]);
    let pk = bls::PublicKey::from_bytes(pk_bytes)?;
    println!("Public Key: {}", pk.to_hex());
    */

    /*
        let apdu_get_app_info = GetAppInfoReq::default();
        let resp = apdu_get_app_info.send(&mut device).await?;
        println!("APDU get app info response: {resp:?}");
    */

    let input = InputLedger {
        unique_pubkey: [0; 48],
        amount: NanoTokens::from(200),
    };

    let unique_pubkey = SecretKey::random().public_key();
    println!("Destination pk: {}", unique_pubkey.to_hex());
    let output = OutputLedger {
        unique_pubkey: unique_pubkey.to_bytes(),
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

    let mut apdu_sign_tx = SignTxReq::new(key_path_0, &spend);
    let resp = apdu_sign_tx.send(&mut device).await;
    println!("APDU sign spend response: {resp:?}");

    let apdu_response = resp?;
    println!("Response data: {:?}", apdu_response.data);

    let length = apdu_response.data[0];
    println!("Signature length: {length}");

    if length as usize != bls::SIG_SIZE || apdu_response.data.len() < bls::SIG_SIZE + 1 {
        bail!(
            "The response data/signature length ({}) doesn't match expected BLS signature length ({})",
            length,
            bls::SIG_SIZE
        );
    }

    let mut sig_bytes = [0u8; bls::SIG_SIZE];
    sig_bytes.copy_from_slice(&apdu_response.data[1..bls::SIG_SIZE + 1]);
    let signature = bls::Signature::from_bytes(sig_bytes);
    println!("Signature: {signature:?}");

    Ok(())
}
