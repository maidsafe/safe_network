// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{
    error::Result,
    spendbook::{check_parent_spends, get_spend_2, validate_spends},
    Node,
};
use bytes::Bytes;
use libp2p::kad::{Record, RecordKey};
use sn_dbc::SignedSpend;
use sn_protocol::storage::{Chunk, DbcAddress, RecordHeader, RecordKind};
use std::collections::HashSet;

impl Node {
    // The `Records` are sent in for validation directly from the Client
    // or during data repub
    pub(super) async fn validate_and_store_record(&mut self, mut record: Record) -> Result<()> {
        let overwrite_attempt = self.network.is_key_present_locally(&record.key).await?;
        let header: RecordHeader = match bincode::deserialize(&record.value) {
            Ok(header) => header,
            Err(_) => {
                error!("Error while deserializing RecordHeader");
                return Ok(());
            }
        };

        match header.kind {
            RecordKind::Chunk => {
                if overwrite_attempt {
                    debug!(
                        "Chunk with key {:?} already exists, not overwriting",
                        record.key
                    );
                    return Ok(());
                }
                let chunk = Chunk::new(Bytes::copy_from_slice(&record.value[RecordHeader::SIZE..]));

                // check if the deserialized value's ChunkAddress matches the record's key
                if record.key != RecordKey::new(chunk.address().name()) {
                    error!(
                        "Record's key does not match with the value's ChunkAddress, ignoring PUT."
                    );
                    return Ok(());
                }
            }
            RecordKind::DbcSpend => {
                let signed_spends = &record.value[RecordHeader::SIZE..];
                let signed_spends: Vec<SignedSpend> = match bincode::deserialize(signed_spends) {
                    Ok(s) => s,
                    Err(e) => {
                        error!("Failed to get spend because deserialization failed: {e:?}");
                        return Ok(());
                    }
                };

                // Prevents someone from crafting large Vecs and slowing down nodes
                if signed_spends.len() > 2 {
                    warn!(
                        "Discarding incoming DbcSpend PUT as it contains more than 2 SignedSpends"
                    );
                }

                // filter out the spends whose DbcAddress does not match with Record::key
                let mut signed_spends = signed_spends.into_iter().filter(|s| {
                    let dbc_addr = DbcAddress::from_dbc_id(s.dbc_id());
                    if record.key != RecordKey::new(dbc_addr.name()) {
                        warn!(
                        "Record's key {:?} does not match with the value's DbcAddress {dbc_addr:?}. Filtering it out.",
                            record.key,
                    );
                        false
                    } else {
                        true
                    }

                }).collect::<Vec<_>>();
                if signed_spends.is_empty() {
                    warn!("No spend with valid Record key found. Ignoring DbcSpend PUT request");
                    return Ok(());
                }
                // get the DbcId; used for validation
                let dbc_id = *signed_spends[0].dbc_id();

                if overwrite_attempt {
                    debug!(
                        "Vec<DbcSpend> with key {:?} already exists, checking if it's the same spend/double spend",
                        record.key
                    );
                    // fetch the locally stored record
                    let local_record = match self.network.get_local_record(&record.key).await? {
                        Some(local_record) => local_record,
                        None => {
                            error!("Local record not found, DiskBackedRecordStore::records went out of sync");
                            return Ok(());
                        }
                    };
                    let local_signed_spends = local_record.value[RecordHeader::SIZE..].to_vec();
                    let local_signed_spends: Vec<SignedSpend> =
                        match bincode::deserialize(&local_signed_spends) {
                            Ok(s) => s,
                            Err(e) => {
                                error!(
                                    "Failed to get spends because deserialization failed: {e:?}"
                                );
                                return Ok(());
                            }
                        };

                    // spends that are not present locally
                    let newly_seen_spends = signed_spends
                        .iter()
                        .filter(|s| !local_signed_spends.contains(s))
                        .cloned()
                        .collect::<HashSet<_>>();

                    // return early if the PUT is for the same local copy
                    if newly_seen_spends.is_empty() {
                        debug!("The overwrite attempt was for the same signed_spends. Ignoring it");
                        return Ok(());
                    } else {
                        // continue with local_spends + new_ones
                        signed_spends = local_signed_spends
                            .into_iter()
                            .chain(newly_seen_spends)
                            .collect();
                    }
                }

                let signed_spends = if signed_spends.len() == 1 {
                    let signed_spend = signed_spends.remove(0);
                    if let Err(err) = check_parent_spends(&self.network, &signed_spend).await {
                        warn!("Invalid Spend Parent {err:?}");
                    }

                    let mut spends = get_spend_2(&self.network, dbc_id).await?;
                    let _ = spends.insert(signed_spend);
                    validate_spends(spends, dbc_id)
                } else {
                    // if we got 2
                    validate_spends(signed_spends.into_iter().collect(), dbc_id)
                };

                // Prepend Kademlia record with a header for storage
                let signed_spends_bytes = match bincode::serialize(&signed_spends) {
                    Ok(b) => b,
                    Err(e) => {
                        error!(
                        "Failed to store spend for {dbc_id:?} because serialization failed: {e:?}"
                    );
                        return Ok(());
                    }
                };

                let record_header = RecordHeader {
                    kind: RecordKind::Chunk,
                };
                let mut record_value = match bincode::serialize(&record_header) {
                    Ok(b) => b,
                    Err(e) => {
                        error!(
                        "Failed to store spend for {dbc_id:?} because RecordHeader serialization failed: {e:?}"
                    );
                        return Ok(());
                    }
                };

                record_value.extend(signed_spends_bytes);
                record.value = record_value;
            }
            RecordKind::Register => {
                if overwrite_attempt {
                    warn!("Overwrite attempt handling for Registers has not been implemented yet. key {:?}", record.key);
                    return Ok(());
                }
            }
        }
        // finally store the Record directly into the local storage
        self.network.put_local_record(record).await?;

        Ok(())
    }
}
