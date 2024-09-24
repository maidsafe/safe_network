// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::{wallet::StoragePaymentResult, Client, Error, Result, WalletClient};
use bls::PublicKey;
use crdts::merkle_reg::MerkleReg;
use libp2p::{
    kad::{Quorum, Record},
    PeerId,
};
use sn_networking::{GetRecordCfg, PutRecordCfg, VerificationKind};
use sn_protocol::{
    storage::{try_serialize_record, RecordKind, RetryStrategy},
    NetworkAddress,
};
use sn_registers::{
    Entry, EntryHash, Error as RegisterError, Permissions, Register, RegisterAddress, RegisterCrdt,
    RegisterOp, SignedRegister,
};
use sn_transfers::{NanoTokens, Payment};
use std::collections::{BTreeSet, HashSet};
use xor_name::XorName;

/// Cached operations made to an offline RegisterCrdt instance are applied locally only,
/// and accumulated until the user explicitly calls 'sync'. The user can
/// switch back to sync with the network for every op by invoking `online` API.
#[derive(Clone, custom_debug::Debug)]
pub struct ClientRegister {
    #[debug(skip)]
    client: Client,
    register: Register,
    /// CRDT data of the Register
    crdt: RegisterCrdt,
    /// Cached operations.
    ops: BTreeSet<RegisterOp>,
}

impl ClientRegister {
    /// Create with specified meta and permission
    pub fn create_register(client: Client, meta: XorName, perms: Permissions) -> Self {
        let register = Register::new(client.signer_pk(), meta, perms);
        let crdt = RegisterCrdt::new(*register.address());
        Self {
            client,
            register,
            crdt,
            ops: BTreeSet::new(),
        }
    }

    /// Create a new Register Locally.
    /// # Arguments
    /// * 'client' - [Client]
    /// * 'meta' - [XorName]
    ///
    /// # Example
    /// ```no_run
    /// # use sn_client::{Client, ClientRegister, Error};
    /// # use bls::SecretKey;
    /// # use xor_name::XorName;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # let mut rng = rand::thread_rng();
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// let address = XorName::random(&mut rng);
    /// // Here we create a ClientRegister
    /// let register = ClientRegister::create(client.clone(), address);
    /// # Ok(())
    /// # }
    /// ```
    pub fn create(client: Client, meta: XorName) -> Self {
        Self::create_register(client, meta, Permissions::default())
    }

    /// Create a new Register locally with a specific address.
    /// # Arguments
    /// * 'client' - [Client]
    /// * 'addr' - [RegisterAddress]
    ///
    /// # Example
    /// ```no_run
    /// # use sn_client::{Client, ClientRegister, Error};
    /// # use bls::SecretKey;
    /// # use sn_protocol::storage::RegisterAddress;
    /// # use xor_name::XorName;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # let mut rng = rand::thread_rng();
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// let address = RegisterAddress::new(XorName::random(&mut rng), client.signer_pk());
    /// // Here we create a ClientRegister
    /// let register = ClientRegister::create_with_addr(client.clone(), address);
    /// # Ok(())
    /// # }
    /// ```
    pub fn create_with_addr(client: Client, addr: RegisterAddress) -> Self {
        let register = Register::new(addr.owner(), addr.meta(), Permissions::default());
        let crdt = RegisterCrdt::new(addr);
        Self {
            client,
            register,
            crdt,
            ops: BTreeSet::new(),
        }
    }

    /// Create a new Register and send it to the Network.
    ///
    /// # Arguments
    /// * 'client' - [Client]
    /// * 'meta' - [XorName]
    /// * 'wallet_client' - A borrowed mutable [WalletClient]
    /// * `verify_store` - A boolean to verify store. Set this to true for mandatory verification.
    /// * 'perms' - [Permissions]
    ///
    /// Return type: Result<(Self, [NanoTokens], [NanoTokens])>
    ///
    /// # Example
    /// ```no_run
    /// # use sn_client::{Client, ClientRegister, Error};
    /// # use bls::SecretKey;
    /// # use xor_name::XorName;
    /// # use tempfile::TempDir;
    /// # use sn_client::WalletClient;
    /// # use sn_registers::Permissions;
    /// # use sn_transfers::{HotWallet, MainSecretKey};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # let mut rng = rand::thread_rng();
    /// # let temporary_path = TempDir::new()?.path().to_owned();
    /// # let main_secret_key = Some(MainSecretKey::new(SecretKey::random()));
    /// # let mut wallet = HotWallet::load_from_path(&temporary_path,main_secret_key)?;
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// let address = XorName::random(&mut rng);
    /// let mut wallet_client = WalletClient::new(client.clone(), wallet);
    /// let permissions = Permissions::default();
    /// // Instantiate a new Register replica from a predefined address.
    /// // The create_online function runs a [sync](ClientRegister::sync) internally.
    /// let (client_register, mut total_cost, mut total_royalties) = ClientRegister::create_online(
    ///         client,
    ///         address,
    ///         &mut wallet_client,
    ///         false,
    ///         permissions,
    ///     ).await?;
    /// # Ok(())
    /// # }
    /// ```
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
        let signed_register = Self::get_register_from_network(&client, address).await?;

        let mut register = Self::create_with_addr(client, address);
        register.merge(&signed_register);

        Ok(register)
    }

    /// Return type: [RegisterAddress]
    ///
    /// # Example
    /// ```no_run
    /// # use sn_client::{Client, ClientRegister, Error};
    /// # use bls::SecretKey;
    /// # use xor_name::XorName;
    /// # use tempfile::TempDir;
    /// # use sn_client::WalletClient;
    /// # use sn_registers::Permissions;
    /// # use sn_transfers::{HotWallet, MainSecretKey};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # let mut rng = rand::thread_rng();
    /// # let temporary_path = TempDir::new()?.path().to_owned();
    /// # let main_secret_key = Some(MainSecretKey::new(SecretKey::random()));
    /// # let mut wallet = HotWallet::load_from_path(&temporary_path,main_secret_key)?;
    /// # let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// # let address = XorName::random(&mut rng);
    /// # let mut wallet_client = WalletClient::new(client.clone(), wallet);
    /// # let permissions = Permissions::default();
    /// // Instantiate a ClientRegister (i.e. with create_online)
    /// let (client_register, mut cost, mut royalties) = ClientRegister::create_online//(...)
    /// # (client,address,&mut wallet_client,false,permissions,).await?;
    /// // From there we can use the address. In this example, we print it out:
    /// println!("REGISTER_ADDRESS={}", client_register.address().to_hex());
    /// # Ok(())
    /// # }
    /// ```
    pub fn address(&self) -> &RegisterAddress {
        self.register.address()
    }

    /// Returns the Owner of the Register.
    ///
    /// Return type: [PublicKey]
    ///
    /// # Example
    /// ```no_run
    /// # use sn_client::{Client, ClientRegister, Error};
    /// # use bls::SecretKey;
    /// # use xor_name::XorName;
    /// # use tempfile::TempDir;
    /// # use sn_client::WalletClient;
    /// # use sn_registers::Permissions;
    /// # use sn_transfers::{HotWallet, MainSecretKey};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # let mut rng = rand::thread_rng();
    /// # let temporary_path = TempDir::new()?.path().to_owned();
    /// # let main_secret_key = Some(MainSecretKey::new(SecretKey::random()));
    /// # let mut wallet = HotWallet::load_from_path(&temporary_path,main_secret_key)?;
    /// # let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// # let address = XorName::random(&mut rng);
    /// # let mut wallet_client = WalletClient::new(client.clone(), wallet);
    /// # let permissions = Permissions::default();
    /// // Instantiate a ClientRegister (i.e. with create_online)
    /// let (client_register, mut cost, mut royalties) = ClientRegister::create_online//(...)
    /// # (client,address,&mut wallet_client,false,permissions,).await?;
    /// // From there we can use the owner. In this example, we print it out:
    /// println!("REGISTER_OWNER={}", client_register.owner().to_hex());
    /// # Ok(())
    /// # }
    /// ```
    pub fn owner(&self) -> PublicKey {
        self.register.owner()
    }

    /// Returns the Permissions of the Register.
    ///
    /// Return type: [Permissions]
    ///
    /// # Example
    /// ```no_run
    /// # use sn_client::{Client, ClientRegister, Error};
    /// # use bls::SecretKey;
    /// # use xor_name::XorName;
    /// # use tempfile::TempDir;
    /// # use sn_client::WalletClient;
    /// # use sn_registers::Permissions;
    /// # use sn_transfers::{HotWallet, MainSecretKey};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # let mut rng = rand::thread_rng();
    /// # let temporary_path = TempDir::new()?.path().to_owned();
    /// # let main_secret_key = Some(MainSecretKey::new(SecretKey::random()));
    /// # let mut wallet = HotWallet::load_from_path(&temporary_path,main_secret_key)?;
    /// # let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// # let address = XorName::random(&mut rng);
    /// let mut wallet_client = WalletClient::new(client.clone(), wallet);
    /// let permissions = Permissions::default();
    /// // Instantiate a ClientRegister (i.e. with create_online)
    /// let (client_register, mut cost, mut royalties) = ClientRegister::create_online//(...)
    /// # (client,address,&mut wallet_client,false,permissions,).await?;
    /// // From there we can use the permissions. In this example, we print it out:
    /// let permissions = client_register.permissions();
    /// println!("REGISTER_PERMS={:?}",permissions);
    /// # Ok(())
    /// # }
    /// ```
    pub fn permissions(&self) -> &Permissions {
        self.register.permissions()
    }

    /// Return the number of items held in the register.
    ///
    /// Return type: u64
    ///
    /// # Example
    /// ```no_run
    /// # use sn_client::{Client, ClientRegister, Error};
    /// # use bls::SecretKey;
    /// # use xor_name::XorName;
    /// # use tempfile::TempDir;
    /// # use sn_client::WalletClient;
    /// # use sn_registers::Permissions;
    /// # use sn_transfers::{HotWallet, MainSecretKey};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # let mut rng = rand::thread_rng();
    /// # let temporary_path = TempDir::new()?.path().to_owned();
    /// # let main_secret_key = Some(MainSecretKey::new(SecretKey::random()));
    /// # let mut wallet = HotWallet::load_from_path(&temporary_path,main_secret_key)?;
    /// # let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// # let address = XorName::random(&mut rng);
    /// # let mut wallet_client = WalletClient::new(client.clone(), wallet);
    /// # let permissions = Permissions::default();
    /// // Instantiate a ClientRegister (i.e. with create_online)
    /// let (client_register, mut cost, mut royalties) = ClientRegister::create_online//(...)
    /// # (client,address,&mut wallet_client,false,permissions,).await?;
    /// // From there we can see the size. In this example, we print it out:
    /// println!("REGISTER_SIZE={}", client_register.size());
    /// # Ok(())
    /// # }
    /// ```
    pub fn size(&self) -> u64 {
        self.crdt.size()
    }

    /// Return a value corresponding to the provided 'hash', if present.
    // No usages found in All Places
    pub fn get(&self, hash: EntryHash) -> Result<&Entry> {
        if let Some(entry) = self.crdt.get(hash) {
            Ok(entry)
        } else {
            Err(RegisterError::NoSuchEntry(hash).into())
        }
    }

    /// Read the last entry, or entries when there are branches, if the register is not empty.
    ///
    /// Return type: [BTreeSet]<([EntryHash], [Entry])>
    ///
    /// # Example
    /// ```no_run
    /// # use sn_client::{Client, ClientRegister, Error};
    /// # use bls::SecretKey;
    /// # use xor_name::XorName;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # let mut rng = rand::thread_rng();
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// let address = XorName::random(&mut rng);
    /// // Read as bytes into the ClientRegister instance
    /// let register = ClientRegister::create(client.clone(), address).read();
    /// # Ok(())
    /// # }
    /// ```
    pub fn read(&self) -> BTreeSet<(EntryHash, Entry)> {
        self.crdt.read()
    }

    /// Write a new value onto the Register atop latest value.
    /// It returns an error if it finds branches in the content/entries; if it is
    /// required to merge/resolve the branches, invoke the `write_merging_branches` API.
    ///
    /// # Arguments
    /// * 'entry' - u8 (i.e .as_bytes)
    ///
    /// # Example
    /// ```no_run
    /// # use sn_client::{Client, ClientRegister, Error};
    /// # use bls::SecretKey;
    /// # use xor_name::XorName;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # let mut rng = rand::thread_rng();
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// let address = XorName::random(&mut rng);
    /// let entry = "Register entry";
    /// // Write as bytes into the ClientRegister instance
    /// let mut register = ClientRegister::create(client.clone(), address).write(entry.as_bytes());
    /// # Ok(())
    /// # }
    /// ```
    pub fn write(&mut self, entry: &[u8]) -> Result<EntryHash> {
        let children = self.crdt.read();
        if children.len() > 1 {
            return Err(Error::ContentBranchDetected(children));
        }

        self.write_atop(entry, &children.into_iter().map(|(hash, _)| hash).collect())
    }

    /// Write a new value onto the Register atop of the latest value.
    /// If there are any branches of content or entries, it automatically merges them.
    /// Leaving the new value as a single latest value on the Register.
    /// Note you can use the `write` API if you need to handle
    /// content/entries branches in a different way.
    ///
    /// # Arguments
    /// * 'entry' - u8 (i.e .as_bytes)
    ///
    /// # Example
    /// ```no_run
    /// # use sn_client::{Client, ClientRegister, Error};
    /// # use bls::SecretKey;
    /// # use xor_name::XorName;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # let mut rng = rand::thread_rng();
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// let address = XorName::random(&mut rng);
    /// let entry = "entry_input_here";
    /// let mut mutable_register = ClientRegister::create(client.clone(), address);
    /// let message = "Register entry";
    /// let register = mutable_register.write_merging_branches(message.as_bytes());
    /// # Ok(())
    /// # }
    /// ```
    pub fn write_merging_branches(&mut self, entry: &[u8]) -> Result<EntryHash> {
        let children: BTreeSet<EntryHash> =
            self.crdt.read().into_iter().map(|(hash, _)| hash).collect();

        self.write_atop(entry, &children)
    }

    /// Write a new value onto the Register atop the set of branches/entries
    /// referenced by the provided list of their corresponding entry hash.
    /// Note you can use `write_merging_branches` API instead if you
    /// want to write atop all exiting branches/entries.
    ///
    /// # Arguments
    /// * 'entry' - u8 (i.e .as_bytes)
    /// * 'children' - [BTreeSet]<[EntryHash]>
    ///
    /// # Example
    /// ```no_run
    /// # use sn_client::{Client, ClientRegister, Error};
    /// # use bls::SecretKey;
    /// # use xor_name::XorName;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # use std::collections::BTreeSet;
    /// let mut rng = rand::thread_rng();
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// let address = XorName::random(&mut rng);
    /// let mut mutable_register = ClientRegister::create(client.clone(), address);
    /// let meta = "Register entry".as_bytes();
    /// let register = mutable_register.write_atop(meta, &BTreeSet::default());
    /// # Ok(())
    /// # }
    /// ```
    pub fn write_atop(
        &mut self,
        entry: &[u8],
        children: &BTreeSet<EntryHash>,
    ) -> Result<EntryHash> {
        // check permissions first
        let public_key = self.client.signer_pk();
        self.register.check_user_permissions(public_key)?;

        let (hash, address, crdt_op) = self.crdt.write(entry.to_vec(), children)?;

        let op = RegisterOp::new(address, crdt_op, self.client.signer());

        let _ = self.ops.insert(op);

        Ok(hash)
    }

    // ********* Online methods  *********

    /// Sync this Register with the replicas on the network.
    /// This will optionally verify the stored Register on the network is the same as the local one.
    /// If payment info is provided it won't try to make the payment.
    ///
    /// # Arguments
    /// * 'wallet_client' - WalletClient
    /// * 'verify_store' - Boolean
    ///
    /// Return type:
    /// Result<([NanoTokens], [NanoTokens])>
    ///
    /// # Example
    /// ```no_run
    /// # use sn_client::{Client, ClientRegister, Error};
    /// # use bls::SecretKey;
    /// # use xor_name::XorName;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # use std::collections::BTreeSet;
    /// # use tempfile::TempDir;
    /// # use sn_client::WalletClient;
    /// # use sn_transfers::{HotWallet, MainSecretKey};
    /// # let mut rng = rand::thread_rng();
    /// # let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// let address = XorName::random(&mut rng);
    /// # let temporary_path = TempDir::new()?.path().to_owned();
    /// # let main_secret_key = Some(MainSecretKey::new(SecretKey::random()));
    /// # let mut wallet = HotWallet::load_from_path(&temporary_path,main_secret_key)?;
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// let mut wallet_client = WalletClient::new(client.clone(), wallet);
    /// // Run sync of a Client Register instance
    /// let mut register =
    ///             ClientRegister::create(client, address).sync(&mut wallet_client, true, None).await?;
    /// # Ok(())
    /// # }
    /// ```
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
            if payment_info.is_some() {
                // we expect this to be a _fresh_ register.
                // It still could have been PUT previously, but we'll do a quick verification
                // instead of thorough one.
                self.client
                    .quickly_check_if_register_stored(*self.address())
                    .await
            } else {
                self.client.verify_register_stored(*self.address()).await
            }
        } else {
            Self::get_register_from_network(&self.client, addr).await
        };

        match reg_result {
            Ok(remote_replica) => {
                self.merge(&remote_replica);
                self.push(verify_store).await?;
            }
            // any error here will result in a repayment of the register
            // TODO: be smart about this and only pay for storage if we need to
            Err(err) => {
                debug!("Failed to get register: {err:?}");
                debug!("Creating Register as it doesn't exist at {addr:?}!");

                // Let's check if the user has already paid for this address first
                if payment_info.is_none() {
                    let net_addr = NetworkAddress::RegisterAddress(addr);
                    let payment_result = self.make_payment(wallet_client, &net_addr).await?;
                    storage_cost = payment_result.storage_cost;
                    royalties_fees = payment_result.royalty_fees;

                    // Get payment proofs needed to publish the Register
                    let (payment, payee) = wallet_client.get_recent_payment_for_addr(&net_addr)?;
                    debug!("payments found: {payment:?}");
                    payment_info = Some((payment, payee));
                }

                // The `creation register` has to come with `payment`.
                // Hence it needs to be `published` to network separately.
                self.publish_register(payment_info, verify_store).await?;
            }
        }

        Ok((storage_cost, royalties_fees))
    }

    /// Push all operations made locally to the replicas of this Register on the network.
    /// This optionally verifies that the stored Register is the same as our local register.
    ///
    /// # Arguments
    /// * 'verify_store' - Boolean
    ///
    /// # Example
    /// ```no_run
    /// # use sn_client::{Client, ClientRegister, Error};
    /// # use bls::SecretKey;
    /// # use xor_name::XorName;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # let mut rng = rand::thread_rng();
    /// let address = XorName::random(&mut rng);
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// // Pass the boolean value to the Client Register instance via .Push()
    /// let mut binding = ClientRegister::create(client, address);
    /// let register = binding.push(false);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn push(&mut self, verify_store: bool) -> Result<()> {
        let ops_len = self.ops.len();
        let address = *self.address();
        if ops_len > 0 {
            if let Err(err) = self.publish_register(None, verify_store).await {
                warn!("Failed to push register {address:?} to network!: {err}");
                return Err(err);
            }

            debug!("Successfully pushed register {address:?} to network!");
        }

        Ok(())
    }

    /// Write a new value onto the Register atop of the latest value.
    /// It returns an error if it finds branches in the content / entries. If so, then it's
    /// required to merge or resolve the branches. In that case, invoke the `write_merging_branches` API.
    ///
    /// # Arguments
    /// * 'entry' - u8 (i.e .as_bytes)
    /// * 'verify_store' - Boolean
    ///
    /// # Example
    /// ```no_run
    /// # use sn_client::{Client, ClientRegister, Error};
    /// # use bls::SecretKey;
    /// # use xor_name::XorName;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # let mut rng = rand::thread_rng();
    /// # let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// let address = XorName::random(&mut rng);
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// let meta = "Register entry".as_bytes();
    /// // Use of the 'write_online' example:
    /// let mut binding = ClientRegister::create(client, address);
    /// let register = binding.write_online(meta,false);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn write_online(&mut self, entry: &[u8], verify_store: bool) -> Result<()> {
        self.write(entry)?;
        self.push(verify_store).await
    }

    /// Write a new value onto the Register atop of the latest value.
    /// If there are branches of content/entries, it will automatically merge them.
    /// This will leave a single new value as the latest entry into the Register.
    /// Note that you can use the `write` API if you need to handle content/entries branches in a different way.
    ///
    /// # Arguments
    /// * 'entry' - u8 (i.e .as_bytes)
    /// * 'verify_store' - Boolean
    ///
    /// # Example
    /// ```no_run
    /// # use sn_client::{Client, ClientRegister, Error};
    /// # use bls::SecretKey;
    /// # use xor_name::XorName;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(),Error>{
    /// # let mut rng = rand::thread_rng();
    /// # let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// let address = XorName::random(&mut rng);
    /// let client = Client::new(SecretKey::random(), None, None, None).await?;
    /// let meta = "Entry".as_bytes();
    /// // Use of the 'write_merging_branches_online':
    /// let mut binding = ClientRegister::create(client, address);
    /// let register = binding.write_merging_branches_online(meta,false);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn write_merging_branches_online(
        &mut self,
        entry: &[u8],
        verify_store: bool,
    ) -> Result<()> {
        self.write_merging_branches(entry)?;
        self.push(verify_store).await
    }

    /// Access the underlying MerkleReg (e.g. for access to history)
    /// NOTE: This API is unstable and may be removed in the future
    pub fn merkle_reg(&self) -> &MerkleReg<Entry> {
        self.crdt.merkle_reg()
    }

    /// Returns the local ops list
    pub fn ops_list(&self) -> &BTreeSet<RegisterOp> {
        &self.ops
    }

    /// Log the crdt DAG in tree structured view
    pub fn log_update_history(&self) -> String {
        self.crdt.log_update_history()
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
    pub async fn publish_register(
        &self,
        payment: Option<(Payment, PeerId)>,
        verify_store: bool,
    ) -> Result<()> {
        let client = self.client.clone();
        let signed_reg = self.get_signed_reg()?;

        let network_address = NetworkAddress::from_register_address(*self.register.address());
        let key = network_address.to_record_key();
        let (record, payee) = match payment {
            Some((payment, payee)) => {
                let record = Record {
                    key: key.clone(),
                    value: try_serialize_record(
                        &(payment, &signed_reg),
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
                    value: try_serialize_record(&signed_reg, RecordKind::Register)?.to_vec(),
                    publisher: None,
                    expires: None,
                };
                (record, None)
            }
        };

        let (record_to_verify, expected_holders) = if verify_store {
            let expected_holders: HashSet<_> = client
                .network
                .get_closest_peers(&network_address, true)
                .await?
                .iter()
                .cloned()
                .collect();
            (
                Some(Record {
                    key,
                    value: try_serialize_record(&signed_reg, RecordKind::Register)?.to_vec(),
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
            retry_strategy: Some(RetryStrategy::Quick),
            target_record: record_to_verify,
            expected_holders,
            is_register: true,
        };
        let put_cfg = PutRecordCfg {
            put_quorum: Quorum::All,
            retry_strategy: Some(RetryStrategy::Balanced),
            use_put_record_to: payee,
            verification: Some((VerificationKind::Network, verification_cfg)),
        };

        // Register edits might exist, so we cannot be sure that just because we get a record back that this should fail
        Ok(client.network.put_record(record, &put_cfg).await?)
    }

    /// Retrieve a `Register` from the Network.
    pub async fn get_register_from_network(
        client: &Client,
        address: RegisterAddress,
    ) -> Result<SignedRegister> {
        debug!("Retrieving Register from: {address}");
        let signed_reg = client.get_signed_register_from_network(address).await?;
        signed_reg.verify_with_address(address)?;
        Ok(signed_reg)
    }

    /// Merge a network fetched copy with the local one.
    /// Note the `get_register_from_network` already verified
    ///   * the fetched register is the same (address) as to the local one
    ///   * the ops of the fetched copy are all signed by the owner
    pub fn merge(&mut self, signed_reg: &SignedRegister) {
        debug!("Merging Register of: {:?}", self.register.address());

        // Take out the difference between local ops and fetched ops
        // note the `difference` functions gives entry that: in a but not in b
        let diff: Vec<_> = signed_reg.ops().difference(&self.ops).cloned().collect();

        // Apply the new ops to local
        for op in diff {
            // in case of deploying error, record then continue to next
            if let Err(err) = self.crdt.apply_op(op.clone()) {
                error!(
                    "Apply op to local Register {:?} failed with {err:?}",
                    self.register.address()
                );
            } else {
                let _ = self.ops.insert(op);
            }
        }
    }

    /// Generate SignedRegister from local copy, so that can be published to network
    fn get_signed_reg(&self) -> Result<SignedRegister> {
        let signature = self.client.sign(self.register.bytes()?);
        Ok(SignedRegister::new(
            self.register.clone(),
            signature,
            self.ops.clone(),
        ))
    }
}
