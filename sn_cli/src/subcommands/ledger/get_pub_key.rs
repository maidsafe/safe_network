// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{derivation_path, ApduINS, ApduP1, ApduP2, APDU_CLA, MAX_REQ_SIZE};

use color_eyre::Result;
use encdec::{self, Decode, Encode};
use ledger_lib::{Device, Error as LedgerLibError};
use ledger_proto::{ApduError, ApduStatic, GenericApdu};
use std::time::Duration;

/// Get public key request (with user confirmation) APDU
#[derive(Clone, Debug, PartialEq, Encode, Decode)]
#[encdec(error = "ApduError")]
pub struct GetPubKeyReq {
    pub dpath: Vec<u8>,
}

impl GetPubKeyReq {
    pub fn new(path: &[u8]) -> Self {
        Self {
            dpath: derivation_path(path),
        }
    }

    pub async fn send<T>(&self, device: &mut T) -> Result<GenericApdu, LedgerLibError>
    where
        T: Device,
    {
        let mut buff = [0u8; MAX_REQ_SIZE];
        device
            .request(self.clone(), &mut buff, Duration::from_secs(100))
            .await
    }
}

impl ApduStatic for GetPubKeyReq {
    const CLA: u8 = APDU_CLA;
    const INS: u8 = ApduINS::GET_PUBLIC_KEY;

    fn p1(&self) -> u8 {
        ApduP1::P1_CONFIRM
    }
    fn p2(&self) -> u8 {
        ApduP2::P2_LAST
    }
}
