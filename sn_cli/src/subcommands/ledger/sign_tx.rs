// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{derivation_path, ApduINS, ApduP1, ApduP2, APDU_CLA, MAX_REQ_SIZE};

use sn_transfers::SpendLedger;

use color_eyre::Result;
use encdec::{self, Decode, Encode};
use ledger_lib::{transport::GenericDevice, Device, Error as LedgerLibError};
use ledger_proto::{ApduError, ApduStatic, GenericApdu};
use std::time::Duration;

/// Sign a trasaction (with user confirmation) APDU
#[derive(Clone, Debug, PartialEq)]
pub struct SignTxReq {
    remaining_bytes: Vec<u8>,
    next_chunk_bytes: Vec<u8>,
    next_p1: u8,
    next_p2: u8,
}

impl SignTxReq {
    pub fn new(path: &[u32], spend: &SpendLedger) -> Self {
        let remaining_bytes = spend.to_bytes();
        println!("LENGTH: {}", remaining_bytes.len());

        Self {
            remaining_bytes,
            next_chunk_bytes: derivation_path(path),
            next_p1: ApduP1::P1_START,
            next_p2: ApduP2::P2_MORE,
        }
    }

    pub async fn send(
        &mut self,
        device: &mut GenericDevice,
    ) -> Result<GenericApdu, LedgerLibError> {
        let mut buff = [0u8; MAX_REQ_SIZE];
        loop {
            match device
                .request(self.clone(), &mut buff, Duration::from_secs(100))
                .await
            {
                // TODO: use embedded ledger safe app exported error/response codes instead of hard-coded numbers
                Err(LedgerLibError::Response(b1, b2)) if b1 == 0x90 && b2 == 0x00 => {
                    let more_to_send = self.next_req();
                    if !more_to_send {
                        break Err(LedgerLibError::Response(b1, b2));
                    }
                }
                other => break other,
            }
        }
    }

    fn next_req(&mut self) -> bool {
        if self.remaining_bytes.is_empty() {
            return false;
        }

        let next_chunk_size = usize::min(MAX_REQ_SIZE / 2, self.remaining_bytes.len());
        // TODO: improve to reduce mem usage
        self.next_chunk_bytes = self.remaining_bytes.clone();
        self.remaining_bytes = self.next_chunk_bytes.split_off(next_chunk_size);

        self.next_p1 += 1;
        if self.remaining_bytes.is_empty() {
            self.next_p2 = ApduP2::P2_LAST;
        } else {
            self.next_p2 = ApduP2::P2_MORE;
        }

        true
    }
}

impl ApduStatic for SignTxReq {
    const CLA: u8 = APDU_CLA;
    const INS: u8 = ApduINS::SIGN_TX;

    fn p1(&self) -> u8 {
        self.next_p1
    }
    fn p2(&self) -> u8 {
        self.next_p2
    }
}

impl Encode for SignTxReq {
    type Error = ApduError;

    /// Fetch encoded length
    fn encode_len(&self) -> Result<usize, Self::Error> {
        Ok(self.next_chunk_bytes.len())
    }

    /// Encode to bytes
    fn encode(&self, buff: &mut [u8]) -> Result<usize, Self::Error> {
        let data = &self.next_chunk_bytes;
        let encode_len = self.encode_len()?;

        // Check buffer length is valid
        if buff.len() < encode_len || encode_len > u8::MAX as usize {
            return Err(ApduError::InvalidLength);
        }

        // Write value
        buff[0..][..data.len()].copy_from_slice(data);
        Ok(encode_len)
    }
}

impl<'a> Decode<'a> for SignTxReq {
    type Output = Self;
    type Error = ApduError;

    // FIXME!
    fn decode(buff: &'a [u8]) -> Result<(Self::Output, usize), Self::Error> {
        // Check buffer length
        if buff.is_empty() {
            return Err(ApduError::InvalidLength);
        }
        let n = buff[0] as usize;
        if n + 1 > buff.len() {
            return Err(ApduError::InvalidLength);
        }

        // Parse string value
        let _s = match core::str::from_utf8(&buff[1..][..n]) {
            Ok(v) => v,
            Err(_) => return Err(ApduError::InvalidUtf8),
        };

        // Return object and parsed length
        Ok((
            Self {
                remaining_bytes: vec![],
                next_chunk_bytes: vec![],
                next_p1: ApduP1::P1_START,
                next_p2: ApduP2::P2_MORE,
            },
            n + 1,
        ))
    }
}
