// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{driver::GetRecordCfg, Network, NetworkError, Result};
use ant_protocol::storage::{LinkedList, LinkedListAddress};
use ant_protocol::{
    storage::{try_deserialize_record, RecordHeader, RecordKind, RetryStrategy},
    NetworkAddress, PrettyPrintRecordKey,
};
use libp2p::kad::{Quorum, Record};

impl Network {
    /// Gets Transactions at TransactionAddress from the Network.
    pub async fn get_transactions(&self, address: LinkedListAddress) -> Result<Vec<LinkedList>> {
        let key = NetworkAddress::from_transaction_address(address).to_record_key();
        let get_cfg = GetRecordCfg {
            get_quorum: Quorum::All,
            retry_strategy: Some(RetryStrategy::Quick),
            target_record: None,
            expected_holders: Default::default(),
            is_register: false,
        };
        let record = self.get_record_from_network(key.clone(), &get_cfg).await?;
        debug!(
            "Got record from the network, {:?}",
            PrettyPrintRecordKey::from(&record.key)
        );

        get_transactions_from_record(&record)
    }
}

pub fn get_transactions_from_record(record: &Record) -> Result<Vec<LinkedList>> {
    let header = RecordHeader::from_record(record)?;
    if let RecordKind::Transaction = header.kind {
        let transactions = try_deserialize_record::<Vec<LinkedList>>(record)?;
        Ok(transactions)
    } else {
        warn!(
            "RecordKind mismatch while trying to retrieve transactions from record {:?}",
            PrettyPrintRecordKey::from(&record.key)
        );
        Err(NetworkError::RecordKindMismatch(RecordKind::Transaction))
    }
}
