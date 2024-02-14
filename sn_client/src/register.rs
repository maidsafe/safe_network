// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{wallet::StoragePaymentResult, Client, Error, Result, WalletClient};

use bls::PublicKey;
use libp2p::{
    kad::{Quorum, Record},
    PeerId,
};
use sn_networking::{GetRecordCfg, PutRecordCfg, VerificationKind};
use sn_protocol::{
    error::Error as ProtocolError,
    messages::RegisterCmd,
    storage::{try_serialize_record, RecordKind, RetryStrategy},
    NetworkAddress,
};

use crdts::merkle_reg::MerkleReg;
use sn_registers::{Entry, EntryHash, Permissions, Register, RegisterAddress, SignedRegister};
use sn_transfers::{NanoTokens, Payment};
use std::collections::{BTreeSet, HashSet, LinkedList};
use xor_name::XorName;

/// Ops made to an offline Register instance are applied locally only,
/// and accumulated till the user explicitly calls 'sync'. The user can
/// switch back to sync with the network for every op by invoking `online` API.
#[derive(Clone)]
pub struct ClientRegister {
    client: Client,
    register: Register,
    ops: LinkedList<RegisterCmd>, // Cached operations.
}

impl ClientRegister {
    /// Central helper func to create a client register
    fn create_register(client: Client, meta: XorName, perms: Permissions) -> Self {
        let public_key = client.signer_pk();

        let register = Register::new(public_key, meta, perms);
        Self {
            client,
            register,
            ops: LinkedList::new(),
        }
    }

    /// Create a new Register Locally.
    pub fn create(client: Client, meta: XorName) -> Self {
        Self::create_register(client, meta, Permissions::new_owner_only())
    }

    /// Create a new Register and send it to the Network.
    pub async fn create_online(
        client: Client,
        meta: XorName,
        wallet_client: &mut WalletClient,
        verify_store: bool,
        perms: Permissions,
    ) -> Result<(Self, NanoTokens, NanoTokens)> {
        let mut reg = Self::create_register(client, meta, perms);
        let (storage_cost, royalties_fees) = reg.sync(wallet_client, verify_store, None).await?;
        Ok((reg, storage_cost, royalties_fees))
    }

    /// Retrieve a Register from the network to work on it offline.
    pub(super) async fn retrieve(client: Client, address: RegisterAddress) -> Result<Self> {
        let register = Self::get_register_from_network(&client, address).await?;

        Ok(Self {
            client,
            register,
            ops: LinkedList::new(),
        })
    }

    pub fn address(&self) -> &RegisterAddress {
        self.register.address()
    }

    /// Return the Owner of the Register.
    pub fn owner(&self) -> PublicKey {
        self.register.owner()
    }

    /// Return the Permissions of the Register.
    pub fn permissions(&self) -> &Permissions {
        self.register.permissions()
    }

    /// Return the number of items held in the register
    pub fn size(&self) -> u64 {
        self.register.size()
    }

    /// Return a value corresponding to the provided 'hash', if present.
    pub fn get(&self, hash: EntryHash) -> Result<&Entry> {
        let entry = self.register.get(hash)?;
        Ok(entry)
    }

    /// Read the last entry, or entries when there are branches, if the register is not empty.
    pub fn read(&self) -> BTreeSet<(EntryHash, Entry)> {
        self.register.read()
    }

    /// Write a new value onto the Register atop latest value.
    /// It returns an error if it finds branches in the content/entries; if it is
    /// required to merge/resolve the branches, invoke the `write_merging_branches` API.
    pub fn write(&mut self, entry: &[u8]) -> Result<()> {
        let children = self.register.read();
        if children.len() > 1 {
            return Err(Error::ContentBranchDetected(children));
        }

        self.write_atop(entry, &children.into_iter().map(|(hash, _)| hash).collect())
    }

    /// Write a new value onto the Register atop latest value.
    /// If there are branches of content/entries, it automatically merges them
    /// all leaving the new value as a single latest value of the Register.
    /// Note you can use `write` API instead if you need to handle
    /// content/entries branches in a diffeerent way.
    pub fn write_merging_branches(&mut self, entry: &[u8]) -> Result<()> {
        let children: BTreeSet<EntryHash> = self
            .register
            .read()
            .into_iter()
            .map(|(hash, _)| hash)
            .collect();

        self.write_atop(entry, &children)
    }

    /// Write a new value onto the Register atop the set of braches/entries
    /// referenced by the provided list of their corresponding entry hash.
    /// Note you can use `write_merging_branches` API instead if you
    /// want to write atop all exiting branches/entries.
    pub fn write_atop(&mut self, entry: &[u8], children: &BTreeSet<EntryHash>) -> Result<()> {
        // check permissions first
        let public_key = self.client.signer_pk();
        self.register.check_user_permissions(public_key)?;

        let (_hash, op) = self
            .register
            .write(entry.into(), children, self.client.signer())?;
        let cmd = RegisterCmd::Edit(op);

        self.ops.push_front(cmd);

        Ok(())
    }

    // ********* Online methods  *********

    /// Sync this Register with the replicas on the network.
    /// This will optionally verify the stored Register on the network is the same as the local one.
    /// If payment info is provided it won't try to make the payment.
    pub async fn sync(
        &mut self,
        wallet_client: &mut WalletClient,
        verify_store: bool,
        mut payment_info: Option<(Payment, PeerId)>,
    ) -> Result<(NanoTokens, NanoTokens)> {
        let addr = *self.address();
        debug!("Syncing Register at {addr:?}!");
        let mut storage_cost = NanoTokens::zero();
        let mut royalties_fees = NanoTokens::zero();
        let reg_result = if verify_store {
            debug!("VERIFYING REGISTER STORED {:?}", self.address());
            let res = self.client.verify_register_stored(*self.address()).await;
            // we need to keep the error here if verifying so we can retry and pay for storage
            // once more below
            match res {
                Ok(r) => Ok(r.register()?),
                Err(error) => Err(error),
            }
        } else {
            Self::get_register_from_network(&self.client, addr).await
        };
        let remote_replica = match reg_result {
            Ok(r) => r,
            // any error here will result in a repayment of the register
            // TODO: be smart about this and only pay for storage if we need to
            Err(err) => {
                debug!("Failed to fetch register: {err:?}");
                debug!("Creating Register as it doesn't exist at {addr:?}!");
                let cmd = RegisterCmd::Create {
                    register: self.register.clone(),
                    signature: self.client.sign(self.register.bytes()?),
                };

                // Let's check if the user has already paid for this address first
                if payment_info.is_none() {
                    let net_addr = sn_protocol::NetworkAddress::RegisterAddress(addr);
                    let payment_result = self.make_payment(wallet_client, &net_addr).await?;
                    storage_cost = payment_result.storage_cost;
                    royalties_fees = payment_result.royalty_fees;

                    // Get payment proofs needed to publish the Register
                    let (payment, payee) = wallet_client.get_payment_for_addr(&net_addr)?;
                    debug!("payments found: {payment:?}");
                    payment_info = Some((payment, payee));
                }

                self.publish_register(cmd, payment_info, verify_store)
                    .await?;
                self.register.clone()
            }
        };
        self.register.merge(remote_replica);
        self.push(verify_store).await?;

        Ok((storage_cost, royalties_fees))
    }

    /// Push all operations made locally to the replicas of this Register on the network.
    /// This optionally verifies that the stored Register is the same as our local register
    pub async fn push(&mut self, verify_store: bool) -> Result<()> {
        let ops_len = self.ops.len();
        if ops_len > 0 {
            let address = *self.address();
            debug!("Pushing {ops_len} cached Register cmds at {address}!");

            // TODO: send them all concurrently
            while let Some(cmd) = self.ops.pop_back() {
                // We don't need to send the payment proofs here since
                // these are all Register mutation cmds which don't require payment.
                let result = self.publish_register(cmd.clone(), None, verify_store).await;

                if let Err(err) = result {
                    warn!("Did not push Register cmd on all nodes in the close group!: {err}");
                    // We keep the cmd for next sync to retry
                    self.ops.push_back(cmd);
                    return Err(err);
                }
            }

            debug!("Successfully pushed {ops_len} Register cmds at {address}!");
        }

        Ok(())
    }

    /// Write a new value onto the Register atop latest value.
    /// It returns an error if it finds branches in the content/entries; if it is
    /// required to merge/resolve the branches, invoke the `write_merging_branches` API.
    pub async fn write_online(&mut self, entry: &[u8], verify_store: bool) -> Result<()> {
        self.write(entry)?;
        self.push(verify_store).await
    }

    /// Write a new value onto the Register atop latest value.
    /// If there are branches of content/entries, it automatically merges them
    /// all leaving the new value as a single latest value of the Register.
    /// Note you can use `write` API instead if you need to handle
    /// content/entries branches in a diffeerent way.
    pub async fn write_merging_branches_online(
        &mut self,
        entry: &[u8],
        verify_store: bool,
    ) -> Result<()> {
        self.write_merging_branches(entry)?;
        self.push(verify_store).await
    }

    /// Write a new value onto the Register atop the set of braches/entries
    /// referenced by the provided list of their corresponding entry hash.
    /// Note you can use `write_merging_branches` API instead if you
    /// want to write atop all exiting branches/entries.
    pub async fn write_atop_online(
        &mut self,
        entry: &[u8],
        children: &BTreeSet<EntryHash>,
        verify_store: bool,
    ) -> Result<()> {
        self.write_atop(entry, children)?;
        self.push(verify_store).await
    }

    /// Access the underlying MerkleReg (e.g. for access to history)
    pub fn merkle_reg(&self) -> &MerkleReg<Entry> {
        &self.register.merkle_reg()
    }

    // ********* Private helpers  *********

    // Make a storage payment for the provided network address
    async fn make_payment(
        &self,
        wallet_client: &mut WalletClient,
        net_addr: &NetworkAddress,
    ) -> Result<StoragePaymentResult> {
        // Let's make the storage payment
        let payment_result = wallet_client
            .pay_for_storage(std::iter::once(net_addr.clone()))
            .await?;
        let cost = payment_result
            .storage_cost
            .checked_add(payment_result.royalty_fees)
            .ok_or(Error::TotalPriceTooHigh)?;

        println!("Successfully made payment of {cost} for a Register (At a cost per record of {cost:?}.)");
        info!("Successfully made payment of {cost} for a Register (At a cost per record of {cost:?}.)");

        if let Err(err) = wallet_client.store_local_wallet() {
            warn!("Failed to store wallet with cached payment proofs: {err:?}");
            println!("Failed to store wallet with cached payment proofs: {err:?}");
        } else {
            println!(
                "Successfully stored wallet with cached payment proofs, and new balance {}.",
                wallet_client.balance()
            );
            info!(
                "Successfully stored wallet with cached payment proofs, and new balance {}.",
                wallet_client.balance()
            );
        }

        Ok(payment_result)
    }

    /// Publish a `Register` command on the network.
    /// If `verify_store` is true, it will verify the Register was stored on the network.
    /// Optionally contains the Payment and the PeerId that we paid to.
    async fn publish_register(
        &self,
        cmd: RegisterCmd,
        payment: Option<(Payment, PeerId)>,
        verify_store: bool,
    ) -> Result<()> {
        let cmd_dst = cmd.dst();
        debug!("Querying existing Register for cmd: {cmd_dst:?}");
        let network_reg = self
            .client
            .get_signed_register_from_network(cmd.dst(), false)
            .await;

        debug!("Publishing Register cmd: {cmd_dst:?}");
        let register = match cmd {
            RegisterCmd::Create {
                register,
                signature,
            } => {
                if let Ok(existing_reg) = network_reg {
                    if existing_reg.owner() != register.owner() {
                        return Err(ProtocolError::RegisterAlreadyClaimed(existing_reg.owner()))?;
                    }
                }
                SignedRegister::new(register, signature)
            }
            RegisterCmd::Edit(op) => {
                let mut reg = network_reg?;
                reg.add_op(op)?;
                reg
            }
        };

        let network_address = NetworkAddress::from_register_address(*register.address());
        let key = network_address.to_record_key();
        let (record, payee) = match payment {
            Some((payment, payee)) => {
                let record = Record {
                    key: key.clone(),
                    value: try_serialize_record(
                        &(payment, &register),
                        RecordKind::RegisterWithPayment,
                    )?
                    .to_vec(),
                    publisher: None,
                    expires: None,
                };
                (record, Some(vec![payee]))
            }
            None => {
                let record = Record {
                    key: key.clone(),
                    value: try_serialize_record(&register, RecordKind::Register)?.to_vec(),
                    publisher: None,
                    expires: None,
                };
                (record, None)
            }
        };

        let (record_to_verify, expected_holders) = if verify_store {
            let expected_holders: HashSet<_> = self
                .client
                .network
                .get_closest_peers(&network_address, true)
                .await?
                .iter()
                .cloned()
                .collect();
            (
                Some(Record {
                    key,
                    value: try_serialize_record(&register, RecordKind::Register)?.to_vec(),
                    publisher: None,
                    expires: None,
                }),
                expected_holders,
            )
        } else {
            (None, Default::default())
        };

        let verification_cfg = GetRecordCfg {
            get_quorum: Quorum::One,
            retry_strategy: Some(RetryStrategy::Balanced),
            target_record: record_to_verify,
            expected_holders,
        };
        let put_cfg = PutRecordCfg {
            put_quorum: Quorum::All,
            retry_strategy: Some(RetryStrategy::Balanced),
            use_put_record_to: payee,
            verification: Some((VerificationKind::Network, verification_cfg)),
        };

        // Register edits might exist so we cannot be sure that just because we get a record back that this should fail
        Ok(self.client.network.put_record(record, &put_cfg).await?)
    }

    // Retrieve a `Register` from the Network.
    async fn get_register_from_network(
        client: &Client,
        address: RegisterAddress,
    ) -> Result<Register> {
        debug!("Retrieving Register from: {address}");
        let reg = client
            .get_signed_register_from_network(address, false)
            .await?;
        reg.verify_with_address(address)?;
        Ok(reg.register()?)
    }
}
