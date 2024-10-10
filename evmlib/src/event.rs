// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::common::{Address, Hash, U256};
use alloy::primitives::{b256, FixedBytes};
use alloy::rpc::types::Log;

// Should be updated when the smart contract changes!
pub(crate) const DATA_PAYMENT_EVENT_SIGNATURE: FixedBytes<32> =
    b256!("f998960b1c6f0e0e89b7bbe6b6fbf3e03e6f08eee5b8430877d8adb8e149d580"); // DevSkim: ignore DS173237

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Topics amount is unexpected. Was expecting 4")]
    TopicsAmountUnexpected,
    #[error("Event signature is missing")]
    EventSignatureMissing,
    #[error("Event signature does not match")]
    EventSignatureDoesNotMatch,
}

/// Struct for the ChunkPaymentEvent emitted by the ChunkPayments smart contract.
#[derive(Debug)]
pub(crate) struct ChunkPaymentEvent {
    pub rewards_address: Address,
    pub amount: U256,
    pub quote_hash: Hash,
}

impl TryFrom<Log> for ChunkPaymentEvent {
    type Error = Error;

    fn try_from(log: Log) -> Result<Self, Self::Error> {
        // Verify the amount of topics
        if log.topics().len() != 4 {
            error!("Topics amount is unexpected. Was expecting 4");
            return Err(Error::TopicsAmountUnexpected);
        }

        let topic0 = log
            .topics()
            .first()
            .ok_or(Error::EventSignatureMissing)
            .inspect_err(|_| error!("Event signature is missing"))?;

        // Verify the event signature
        if topic0 != &DATA_PAYMENT_EVENT_SIGNATURE {
            error!(
                "Event signature does not match. Expected: {:?}, got: {:?}",
                DATA_PAYMENT_EVENT_SIGNATURE, topic0
            );
            return Err(Error::EventSignatureDoesNotMatch);
        }

        // Extract the data
        let rewards_address = Address::from_slice(&log.topics()[1][12..]);
        let amount = U256::from_be_slice(&log.topics()[2][12..]);
        let quote_hash = Hash::from_slice(log.topics()[3].as_slice());

        Ok(Self {
            rewards_address,
            amount,
            quote_hash,
        })
    }
}
