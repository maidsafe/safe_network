// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::rand::{distributions::Standard, Rng, RngCore};
use crate::wallet::{Error, Result};

use bls::{serde_impl::SerdeSecret, PublicKey, SecretKey, PK_SIZE};
use serde::{Deserialize, Serialize};
use std::fmt;

/// This is used to generate a new UniquePubkey
/// from a MainPubkey, and the corresponding
/// DerivedSecretKey from the MainSecretKey of that MainPubkey.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize, Hash)]
pub struct DerivationIndex(pub [u8; 32]);

impl fmt::Debug for DerivationIndex {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "{:02x}{:02x}{:02x}..",
            self.0[0], self.0[1], self.0[2]
        )
    }
}

impl DerivationIndex {
    // generates a random derivation index
    pub fn random(rng: &mut impl RngCore) -> DerivationIndex {
        let mut bytes = [0u8; 32];
        rng.fill_bytes(&mut bytes);
        DerivationIndex(bytes)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize, Hash)]
pub struct UniquePubkey(PublicKey);

impl UniquePubkey {
    pub fn new<G: Into<PublicKey>>(public_key: G) -> Self {
        Self(public_key.into())
    }

    pub fn to_bytes(&self) -> [u8; bls::PK_SIZE] {
        self.0.to_bytes()
    }

    /// Returns `true` if the signature matches the message.
    pub fn verify<M: AsRef<[u8]>>(&self, sig: &bls::Signature, msg: M) -> bool {
        self.0.verify(sig, msg)
    }

    pub fn public_key(&self) -> PublicKey {
        self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0.to_bytes())
    }

    pub fn from_hex<T: AsRef<[u8]>>(hex: T) -> Result<Self> {
        let public_key = bls_public_from_hex(hex)?;
        Ok(Self::new(public_key))
    }
}

/// This is the key that unlocks the value of a CashNote.
/// Holding this key gives you access to the tokens of the
/// CashNote with the corresponding UniquePubkey.
/// Like with the keys to your house or a safe, this is not something you share publicly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivedSecretKey(SerdeSecret<SecretKey>);

impl DerivedSecretKey {
    pub fn new<S: Into<SecretKey>>(secret_key: S) -> Self {
        Self(SerdeSecret(secret_key.into()))
    }

    /// This is the unique identifier of the CashNote that
    /// this instance of CashNote secret key unlocks.
    /// The CashNote does not exist until someone has sent tokens to it.
    pub fn unique_pubkey(&self) -> UniquePubkey {
        UniquePubkey(self.0.public_key())
    }

    pub(crate) fn sign(&self, msg: &[u8]) -> bls::Signature {
        self.0.sign(msg)
    }
}

/// This is the MainPubkey to which tokens are send.
///
/// The MainPubkey may be published and multiple payments sent to this address by various parties.  
/// It is useful for accepting donations, for example.
///
/// The CashNote can only be spent by the party holding the MainSecretKey that corresponds to the
/// MainPubkey, ie the CashNote recipient.
///
/// This MainPubkey is only a client/wallet concept. It is NOT actually used in the transaction
/// and never seen by the spentbook nodes.
///
/// The UniquePubkey used in the transaction is derived from this MainPubkey using a random
/// derivation index, which is stored in derivation_index.
///
/// When someone wants to send tokens to this MainPubkey,
/// they generate the id of the CashNote - the UniquePubkey - that shall hold the tokens.
/// The UniquePubkey is generated from this MainPubkey, and only the sender
/// will at this point know that the UniquePubkey is related to this MainPubkey.
/// When creating the CashNote using that UniquePubkey, the sender will also include the
/// DerivationIndex that was used to generate the UniquePubkey, so that the recipient behind
/// the MainPubkey can also see that the UniquePubkey is related to this MainPubkey.
/// The recipient can then use the received DerivationIndex to generate the DerivedSecretKey
/// corresponding to that UniquePubkey, and thus unlock the value of the CashNote by using that DerivedSecretKey.
#[derive(Copy, Debug, PartialEq, Eq, Ord, PartialOrd, Clone, Serialize, Deserialize, Hash)]
pub struct MainPubkey(pub PublicKey);

impl MainPubkey {
    pub fn new(public_key: PublicKey) -> Self {
        Self(public_key)
    }

    /// Verify that the signature is valid for the message.
    pub fn verify(&self, sig: &bls::Signature, msg: &[u8]) -> bool {
        self.0.verify(sig, msg)
    }

    /// Generate a new UniquePubkey from provided DerivationIndex.
    /// This is supposed to be a unique identifier of a CashNote.
    /// A new CashNote id is generated by someone who wants to send tokens to the MainPubkey.
    /// When they create the new CashNote they will use this id, but that only works if this id was never used before.
    pub fn new_unique_pubkey(&self, index: &DerivationIndex) -> UniquePubkey {
        UniquePubkey(self.0.derive_child(&index.0))
    }

    pub fn to_bytes(self) -> [u8; PK_SIZE] {
        self.0.to_bytes()
    }

    // Get the underlying PublicKey
    pub fn public_key(&self) -> PublicKey {
        self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0.to_bytes())
    }

    pub fn from_hex<T: AsRef<[u8]>>(hex: T) -> Result<Self> {
        let public_key = bls_public_from_hex(hex)?;
        Ok(Self::new(public_key))
    }
}

/// A CashNote MainSecretKey is held by anyone who wants to
/// send or receive tokens using CashNotes. It is held privately
/// and not shared with anyone.
///
/// The secret MainSecretKey has a static MainPubkey, which
/// is shared with others in order to receive payments.
/// With this MainSecretKey, new DerivedSecretKey:UniquePubkey pairs can be generated.
pub struct MainSecretKey(SerdeSecret<SecretKey>);

impl MainSecretKey {
    /// Create a new MainSecretKey from a bls SecretKey.
    pub fn new(secret_key: SecretKey) -> Self {
        Self(SerdeSecret(secret_key))
    }

    /// Get the secret key.
    pub fn secret_key(&self) -> &SecretKey {
        &self.0
    }

    /// This is the static public address which is shared with others, and
    /// to which payments can be made by getting a new unique identifier for a CashNote to be created.
    pub fn main_pubkey(&self) -> MainPubkey {
        MainPubkey(self.0.public_key())
    }

    /// Sign a message with the main key.
    pub fn sign(&self, msg: &[u8]) -> bls::Signature {
        self.0.sign(msg)
    }

    /// Derive the key - the DerivedSecretKey - corresponding to a UniquePubkey
    /// which was also derived using the same DerivationIndex.
    ///
    /// When someone wants to send tokens to the MainPubkey of this MainSecretKey,
    /// they generate the id of the CashNote - the UniquePubkey - that shall hold the tokens.
    /// The recipient of the tokens, is the person/entity that holds this MainSecretKey.
    ///
    /// The created CashNote contains the derivation index that was used to
    /// generate that very UniquePubkey.
    ///
    /// When passing the derivation index to this function (`fn derive_key`),
    /// a DerivedSecretKey is generated corresponding to the UniquePubkey. This DerivedSecretKey can unlock the CashNote of that
    /// UniquePubkey, thus giving access to the tokens it holds.
    /// By that, the recipient has received the tokens from the sender.
    pub fn derive_key(&self, index: &DerivationIndex) -> DerivedSecretKey {
        DerivedSecretKey::new(self.0.inner().derive_child(&index.0))
    }

    /// Represent as bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.0.to_bytes().to_vec()
    }

    pub fn random() -> Self {
        Self::new(bls::SecretKey::random())
    }

    /// Create a randomly generated MainSecretKey.
    pub fn random_from_rng(rng: &mut impl RngCore) -> Self {
        let sk: SecretKey = rng.sample(Standard);
        Self::new(sk)
    }

    pub fn random_derived_key(&self, rng: &mut impl RngCore) -> DerivedSecretKey {
        self.derive_key(&DerivationIndex::random(rng))
    }
}

/// Construct a BLS public key from a hex-encoded string.
fn bls_public_from_hex<T: AsRef<[u8]>>(hex: T) -> Result<bls::PublicKey> {
    let bytes = hex::decode(hex).map_err(|_| Error::FailedToDecodeHexToKey)?;
    let bytes_fixed_len: [u8; bls::PK_SIZE] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| Error::FailedToParseBlsKey)?;
    let pk = bls::PublicKey::from_bytes(bytes_fixed_len)?;
    Ok(pk)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pubkeys_hex_conversion() -> eyre::Result<()> {
        let sk = bls::SecretKey::random();
        let pk = sk.public_key();
        let main_pubkey = MainPubkey::new(pk);
        let unique_pubkey =
            main_pubkey.new_unique_pubkey(&DerivationIndex::random(&mut rand::thread_rng()));

        let main_pubkey_hex = main_pubkey.to_hex();
        let unique_pubkey_hex = unique_pubkey.to_hex();

        let main_pubkey_from_hex = MainPubkey::from_hex(main_pubkey_hex)?;
        let unique_pubkey_from_hex = UniquePubkey::from_hex(unique_pubkey_hex)?;

        assert_eq!(main_pubkey, main_pubkey_from_hex);
        assert_eq!(unique_pubkey, unique_pubkey_from_hex);
        Ok(())
    }
}
