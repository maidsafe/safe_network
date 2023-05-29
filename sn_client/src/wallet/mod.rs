// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

pub(crate) mod send_client;
pub(crate) mod verifying_client;

use super::Client;

use sn_dbc::{Dbc, PublicAddress, Token};
use sn_domain::wallet::{Error, LocalWallet, Result, SendWallet};
use sn_protocol::NetworkAddress;

use bls::SecretKey;
use merkletree::{merkle::MerkleTree, store::VecStore};
use std::iter::Iterator;

/// A wallet client can be used to send and
/// receive tokens to/from other wallets.
pub struct WalletClient<W: SendWallet> {
    client: Client,
    wallet: W,
}

impl<W: SendWallet> WalletClient<W> {
    /// Create a new wallet client.
    pub fn new(client: Client, wallet: W) -> Self {
        Self { client, wallet }
    }

    /// Send tokens to nodes closest to the data we want to store and pay for.
    pub async fn pay_for_storage(
        &mut self,
        chunks: impl Iterator<Item = &NetworkAddress>,
    ) -> Result<Dbc> {
        // FIXME: calculate the amount to pay to each node, perhaps just 1 nano to begin with.
        let amount = Token::from_nano(1);

        // Let's build the Merkle-tree to obtain the reason-hash
        let tree_leaves: Vec<MerkleTreeItem> = chunks
            .map(|c| MerkleTreeItem::from_slice(&c.as_bytes()))
            .collect();
        let tree = MerkleTree::<_, _, VecStore<MerkleTreeItem>>::from_data(tree_leaves)
            .map_err(|err| Error::StoragePaymentReason(err.to_string()))?;

        // The reason hash is set to be the root of the merkle-tree of chunks to pay for
        let reason_hash = tree.root().0.into();
        println!(">>>> Reason hash: {reason_hash:?}");

        // FIXME: calculate closest nodes to pay for storage
        let to = PublicAddress::new(SecretKey::random().public_key());

        let dbcs = self
            .wallet
            .send(vec![(amount, to)], &self.client, Some(reason_hash))
            .await?;

        match &dbcs[..] {
            [info, ..] => Ok(info.dbc.clone()),
            [] => Err(Error::CouldNotSendTokens(
                "No DBCs were returned from the wallet.".into(),
            )),
        }
    }

    /// Send tokens to another wallet.
    pub async fn send(&mut self, amount: Token, to: PublicAddress) -> Result<Dbc> {
        let dbcs = self
            .wallet
            .send(vec![(amount, to)], &self.client, None)
            .await?;
        match &dbcs[..] {
            [info, ..] => Ok(info.dbc.clone()),
            [] => Err(Error::CouldNotSendTokens(
                "No DBCs were returned from the wallet.".into(),
            )),
        }
    }

    /// Return the wallet.
    pub fn into_wallet(self) -> W {
        self.wallet
    }
}

/// Use the client to send a DBC from a local wallet to an address.
pub async fn send(from: LocalWallet, amount: Token, to: PublicAddress, client: &Client) -> Dbc {
    if amount.as_nano() == 0 {
        panic!("Amount must be more than zero.");
    }

    let mut wallet_client = WalletClient::new(client.clone(), from);
    let new_dbc = wallet_client
        .send(amount, to)
        .await
        .expect("Tokens shall be successfully sent.");

    let mut wallet = wallet_client.into_wallet();
    wallet
        .store()
        .await
        .expect("Wallet shall be successfully stored.");
    wallet
        .store_created_dbc(new_dbc.clone())
        .await
        .expect("Created dbc shall be successfully stored.");

    new_dbc
}

use merkletree::{
    hash::{Algorithm, Hashable},
    merkle::Element,
};
use std::fmt;
use tiny_keccak::{Hasher, Sha3};

const SIZE: usize = 32;
#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Debug, Default)]
struct MerkleTreeItem([u8; SIZE]);

impl AsRef<[u8]> for MerkleTreeItem {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Element for MerkleTreeItem {
    fn byte_len() -> usize {
        SIZE
    }

    fn from_slice(bytes: &[u8]) -> Self {
        assert_eq!(bytes.len(), Self::byte_len());
        let mut el = [0u8; SIZE];
        el[..].copy_from_slice(bytes);
        MerkleTreeItem(el)
    }

    fn copy_to_slice(&self, bytes: &mut [u8]) {
        bytes.copy_from_slice(&self.0);
    }
}

struct TestSha256Hasher {
    engine: Sha3,
}

impl TestSha256Hasher {
    fn new() -> TestSha256Hasher {
        TestSha256Hasher {
            engine: Sha3::v256(),
        }
    }
}

impl fmt::Debug for TestSha256Hasher {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("Sha256Hasher")
    }
}

impl Default for TestSha256Hasher {
    fn default() -> Self {
        TestSha256Hasher::new()
    }
}

impl std::hash::Hasher for TestSha256Hasher {
    fn finish(&self) -> u64 {
        unimplemented!()
    }

    fn write(&mut self, bytes: &[u8]) {
        self.engine.update(bytes)
    }
}

impl Hashable<TestSha256Hasher> for MerkleTreeItem {
    // Required method
    fn hash(&self, state: &mut TestSha256Hasher) {}
}

impl Algorithm<MerkleTreeItem> for TestSha256Hasher {
    fn hash(&mut self) -> MerkleTreeItem {
        let mut result = MerkleTreeItem::default();
        let item_size = result.0.len();

        let sha3 = self.engine.clone();
        let mut hash = [0u8; 32];
        sha3.finalize(&mut hash);
        let hash_output = hash.to_vec();

        if item_size < hash_output.len() {
            result
                .0
                .copy_from_slice(&hash_output.as_slice()[0..item_size]);
        } else {
            result.0.copy_from_slice(hash_output.as_slice())
        }
        result
    }
}
