// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[macro_use]
extern crate tracing;

mod cashnotes;
mod error;
mod genesis;
mod transfers;
mod wallet;

pub(crate) use cashnotes::{Input, TransactionBuilder};

/// Types used in the public API
pub use cashnotes::{
    CashNote, DerivationIndex, DerivedSecretKey, Hash, MainPubkey, MainSecretKey, NanoTokens,
    SignedSpend, Spend, SpendAddress, Transaction, UniquePubkey, UnsignedTransfer,
};
pub use error::{Result, TransferError};
pub use transfers::{CashNoteRedemption, OfflineTransfer, Transfer};

/// Utilities exposed
pub use genesis::{
    calculate_royalties_fee, create_faucet_wallet, create_first_cash_note_from_key,
    get_faucet_data_dir, is_genesis_parent_tx, is_genesis_spend, load_genesis_wallet,
    Error as GenesisError, GENESIS_CASHNOTE, GENESIS_CASHNOTE_SK, NETWORK_ROYALTIES_PK,
};
pub use wallet::bls_secret_from_hex;
pub use wallet::{
    Error as WalletError, HotWallet, Payment, PaymentQuote, Result as WalletResult, WatchOnlyWallet,
};

// re-export crates used in our public API
pub use bls::{self, rand, Ciphertext, Signature};

/// This is a helper module to make it a bit easier
/// and regular for API callers to instantiate
/// an Rng when calling sn_transfers methods that require
/// them.
pub mod rng {
    use crate::rand::{
        rngs::{StdRng, ThreadRng},
        SeedableRng,
    };
    use tiny_keccak::{Hasher, Sha3};

    pub fn thread_rng() -> ThreadRng {
        crate::rand::thread_rng()
    }

    pub fn from_seed(seed: <StdRng as SeedableRng>::Seed) -> StdRng {
        StdRng::from_seed(seed)
    }

    // Using hash to covert `Vec<u8>` into `[u8; 32]',
    // and using it as seed to generate a determined Rng.
    pub fn from_vec(vec: &[u8]) -> StdRng {
        let mut sha3 = Sha3::v256();
        sha3.update(vec);
        let mut hash = [0u8; 32];
        sha3.finalize(&mut hash);

        from_seed(hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::from_vec;

    #[test]
    fn confirm_generating_same_key() {
        let rng_seed = b"testing generating same key";
        let content = b"some context to try with";

        let mut rng_1 = from_vec(rng_seed);
        let reward_key_1 = MainSecretKey::random_from_rng(&mut rng_1);
        let sig = reward_key_1.sign(content);

        let mut rng_2 = from_vec(rng_seed);
        let reward_key_2 = MainSecretKey::random_from_rng(&mut rng_2);

        assert!(reward_key_2.main_pubkey().verify(&sig, content));
    }
}
