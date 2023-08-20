// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::Node;
use sn_dbc::{SignedSpend, Token};
use sn_protocol::{
    error::{Error, Result},
    messages::ReplicatedData,
    storage::{try_deserialize_record, ChunkWithPayment, DbcAddress, RecordHeader, RecordKind},
    NetworkAddress, PrettyPrintRecordKey,
};
use sn_registers::SignedRegister;

impl Node {
    /// Get the current storecost in nanos from our local kademlia store
    /// Returns cost and our node's signature over that cost
    pub(crate) async fn current_storecost(&self) -> Result<Token> {
        let cost = self
            .network
            .get_local_storecost()
            .await
            .map_err(|_| Error::GetStoreCostFailed)?;

        Ok(cost)
    }

    pub(crate) async fn get_spend_from_network(
        &self,
        address: DbcAddress,
        re_attempt: bool,
    ) -> Result<SignedSpend> {
        let key = NetworkAddress::from_dbc_address(address).to_record_key();
        let record = self
            .network
            .get_record_from_network(key, None, re_attempt, false)
            .await
            .map_err(|_| Error::SpendNotFound(address))?;
        debug!(
            "Got record from the network, {:?}",
            PrettyPrintRecordKey::from(record.key.clone())
        );
        let header =
            RecordHeader::from_record(&record).map_err(|_| Error::SpendNotFound(address))?;

        if let RecordKind::DbcSpend = header.kind {
            match try_deserialize_record::<Vec<SignedSpend>>(&record)
                .map_err(|_| Error::SpendNotFound(address))?
                .as_slice()
            {
                [one, two, ..] => {
                    error!("Found double spend for {address:?}");
                    Err(Error::DoubleSpendAttempt(
                        Box::new(one.to_owned()),
                        Box::new(two.to_owned()),
                    ))
                }
                [one] => {
                    trace!("Spend get for address: {address:?} successful");
                    Ok(one.clone())
                }
                _ => {
                    trace!("Found no spend for {address:?}");
                    Err(Error::SpendNotFound(address))
                }
            }
        } else {
            error!("RecordKind mismatch while trying to retrieve a Vec<SignedSpend>");
            Err(Error::RecordKindMismatch(RecordKind::DbcSpend))
        }
    }

    pub(crate) async fn get_replicated_data(
        &self,
        address: NetworkAddress,
    ) -> Result<ReplicatedData> {
        let error = Error::ReplicatedDataNotFound {
            holder: Box::new(NetworkAddress::from_peer(self.network.peer_id)),
            address: Box::new(address.clone()),
        };

        let record_key = address.as_record_key().ok_or(error.clone())?;
        let record = self
            .network
            .get_record_from_network(record_key, None, false, false)
            .await
            .map_err(|_| error.clone())?;
        let header = RecordHeader::from_record(&record).map_err(|_| error.clone())?;

        match header.kind {
            RecordKind::Chunk => {
                let chunk_with_payment =
                    try_deserialize_record::<ChunkWithPayment>(&record).map_err(|_| error)?;
                trace!(
                    "Replicating chunk with address {:?}",
                    chunk_with_payment.chunk.address()
                );

                Ok(ReplicatedData::Chunk(chunk_with_payment))
            }

            RecordKind::DbcSpend => {
                let spends =
                    try_deserialize_record::<Vec<SignedSpend>>(&record).map_err(|_| error)?;
                Ok(ReplicatedData::DbcSpend(spends))
            }
            RecordKind::Register => {
                let register =
                    try_deserialize_record::<SignedRegister>(&record).map_err(|_| error)?;
                Ok(ReplicatedData::Register(register))
            }
        }
    }
}
