// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

mod conn;
mod get_app_info;
mod get_pub_key;
mod sign_tx;

use conn::connect_to_device;
use get_app_info::GetAppInfoReq;
use get_pub_key::GetPubKeyReq;
use sign_tx::SignTxReq;

use sn_transfers::SpendLedger;

use bls::{PublicKey, Signature, PK_SIZE, SIG_SIZE};
use color_eyre::{eyre::bail, Result};
use ledger_lib::transport::GenericDevice;

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

// We use purpose 12381 as defined in EIP2334, and coin type == 1 as defined in SLIP-0044 for all testnets.
const PURPOSE: u32 = 12381;
const COIN_TYPE: u32 = 1;

// Encode key derivation path from a given use type.
// Currently we only derive to the 3rd level of EIP2334 ('account' component in the path), but we
// shall eventually allow the user to choose the 4th level ('use' component in the path) to differentiate
// addresses used for files/storage payments from those used by the user for regular SNT transfers, etc.
fn serialised_derivation_path(account: Option<u32>) -> Vec<u8> {
    let account_index = account.unwrap_or(0u32);
    let use_index = 0u32;
    let dpath = &[PURPOSE, COIN_TYPE, account_index, use_index];

    let mut serialised = vec![dpath.len() as u8];
    for item in dpath {
        serialised.extend(item.to_be_bytes());
    }

    println!("Using key derivation path: {dpath:?}");

    serialised
}

pub struct LedgerSntWallet {
    device: GenericDevice,
}

impl LedgerSntWallet {
    pub async fn new() -> Result<Self> {
        println!("** Using Ledger hardware wallet **");
        let device = connect_to_device().await?;
        Ok(Self { device })
    }

    pub async fn get_addr(&mut self, account: Option<u32>) -> Result<PublicKey> {
        let apdu_get_public_key = GetPubKeyReq::new(account);
        let apdu_response = apdu_get_public_key.send(&mut self.device).await?;
        println!("APDU get pub key response: {apdu_response:?}");

        let len = apdu_response.data.len();
        if len < PK_SIZE {
            bail!(
                "The response data length ({len}) doesn't match expected BLS pk length ({PK_SIZE})"
            );
        }

        let mut pk_bytes = [0u8; PK_SIZE];
        pk_bytes.copy_from_slice(&apdu_response.data[0..PK_SIZE]);
        let pk = PublicKey::from_bytes(pk_bytes)?;
        Ok(pk)
    }

    pub async fn app_info(&mut self) -> Result<()> {
        let apdu_get_app_info = GetAppInfoReq::default();
        let resp = apdu_get_app_info.send(&mut self.device).await?;
        println!("APDU get app info response: {resp:?}");

        Ok(())
    }

    pub async fn sign_spend(
        &mut self,
        account: Option<u32>,
        spend: &SpendLedger,
    ) -> Result<(Signature, PublicKey)> {
        let mut apdu_sign_tx = SignTxReq::new(account, spend);
        let resp = apdu_sign_tx.send(&mut self.device).await;
        //println!("APDU sign spend response: {resp:?}");

        let apdu_response = resp?;
        //println!("Response data: {:?}", apdu_response.data);

        let len = apdu_response.data.len();
        if len < SIG_SIZE + PK_SIZE {
            bail!("The response data length ({len}) doesn't match expected BLS signature + PK length ({})", SIG_SIZE + PK_SIZE);
        }

        let mut sig_bytes = [0u8; SIG_SIZE];
        sig_bytes.copy_from_slice(&apdu_response.data[0..SIG_SIZE]);
        let ledger_signature = Signature::from_bytes(sig_bytes)?;

        let mut pk_bytes = [0u8; PK_SIZE];
        pk_bytes.copy_from_slice(&apdu_response.data[SIG_SIZE..SIG_SIZE + PK_SIZE]);
        let pk = PublicKey::from_bytes(pk_bytes)?;

        Ok((ledger_signature, pk))
    }
}
