// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::Node;
use libp2p::kad::RecordKey;
use sn_dbc::SignedSpend;
use sn_protocol::{
    error::{Error, Result},
    messages::ReplicatedData,
    storage::{
        try_deserialize_record, Chunk, ChunkAddress, ChunkWithPayment, DbcAddress, RecordHeader,
        RecordKind, RegisterAddress,
    },
    NetworkAddress,
};
use sn_registers::SignedRegister;

impl Node {
    pub(crate) async fn get_chunk_from_network(&self, address: ChunkAddress) -> Result<Chunk> {
        let record = self
            .network
            .get_record_from_network(RecordKey::new(address.name()))
            .await
            .map_err(|_| Error::ChunkNotFound(address))?;
        debug!("Got record from the network, {:?}", record.key);
        let header =
            RecordHeader::from_record(&record).map_err(|_| Error::ChunkNotFound(address))?;

        if let RecordKind::Chunk = header.kind {
            let chunk_with_payment = try_deserialize_record::<ChunkWithPayment>(&record)
                .map_err(|_| Error::ChunkNotFound(address))?;
            Ok(chunk_with_payment.chunk)
        } else {
            error!("RecordKind mismatch while trying to retrieve a chunk");
            Err(Error::RecordKindMismatch(RecordKind::Chunk))
        }
    }

    pub(crate) async fn get_signed_register_from_network(
        &self,
        address: RegisterAddress,
    ) -> Result<SignedRegister> {
        let record = self
            .network
            .get_record_from_network(RecordKey::new(address.name()))
            .await
            .map_err(|_| Error::RegisterNotFound(address))?;
        debug!("Got record from the network, {:?}", record.key);
        let header =
            RecordHeader::from_record(&record).map_err(|_| Error::RegisterNotFound(address))?;

        if let RecordKind::Register = header.kind {
            let register = try_deserialize_record::<SignedRegister>(&record)
                .map_err(|_| Error::RegisterNotFound(address))?;
            Ok(register)
        } else {
            error!("RecordKind mismatch while trying to retrieve a chunk");
            Err(Error::RecordKindMismatch(RecordKind::Register))
        }
    }

    pub(crate) async fn get_spend_from_network(&self, address: DbcAddress) -> Result<SignedSpend> {
        let record = self
            .network
            .get_record_from_network(RecordKey::new(address.name()))
            .await
            .map_err(|_| Error::SpendNotFound(address))?;
        debug!("Got record from the network, {:?}", record.key);
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
            holder: NetworkAddress::from_peer(self.network.peer_id),
            address: address.clone(),
        };

        let record_key = address.as_record_key().ok_or(error.clone())?;
        let record = self
            .network
            .get_record_from_network(record_key)
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
