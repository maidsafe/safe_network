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

/// Types used in the public API
pub use cashnotes::{
    CashNote, DerivationIndex, DerivedSecretKey, Hash, MainPubkey, MainSecretKey, NanoTokens,
    SignedSpend, Spend, SpendAddress, SpendReason, UniquePubkey,
};
pub use error::{Result, TransferError};
/// Utilities exposed
pub use genesis::{
    calculate_royalties_fee, create_first_cash_note_from_key, get_faucet_data_dir, get_genesis_sk,
    is_genesis_spend, load_genesis_wallet, Error as GenesisError, GENESIS_CASHNOTE,
    GENESIS_INPUT_DERIVATION_INDEX, GENESIS_OUTPUT_DERIVATION_INDEX, GENESIS_PK,
    GENESIS_SPEND_UNIQUE_KEY, TOTAL_SUPPLY,
};
pub use transfers::{CashNoteRedemption, SignedTransaction, Transfer, UnsignedTransaction};
pub use wallet::{
    bls_secret_from_hex, wallet_lockfile_name, Error as WalletError, HotWallet, Payment,
    PaymentQuote, QuotingMetrics, Result as WalletResult, WalletApi, WatchOnlyWallet,
    QUOTE_EXPIRATION_SECS, WALLET_DIR_NAME,
};

use bls::SecretKey;
use lazy_static::lazy_static;

/// The following PKs shall be updated to match its correspondent SKs before the formal release
///
/// Foundation wallet public key (used to receive initial disbursment from the genesis wallet)
const DEFAULT_FOUNDATION_PK_STR: &str = "8f73b97377f30bed96df1c92daf9f21b4a82c862615439fab8095e68860a5d0dff9f97dba5aef503a26c065e5cb3c7ca"; // DevSkim: ignore DS173237
/// Public key where network royalties payments are expected to be made to.
const DEFAULT_NETWORK_ROYALTIES_STR: &str = "b4243ec9ceaec374ef992684cd911b209758c5de53d1e406b395bc37ebc8ce50e68755ea6d32da480ae927e1af4ddadb"; // DevSkim: ignore DS173237
/// Public key where payment forward to be targeted.
const DEFAULT_PAYMENT_FORWARD_STR: &str = "a585839f0502713a0ed6a327f3bd0c301f9e8fe298c93dd00ed7869d8e6804244f0d3014e90df45cd344a7ccd702865c"; // DevSkim: ignore DS173237
/// Default secrect key where payment forward to be targeted, for backward compatible purpose only.
const DEFAULT_PAYMENT_FORWARD_SK_STR: &str =
    "49113d2083f57a976076adbe85decb75115820de1e6e74b47e0429338cef124a"; // DevSkim: ignore DS173237

lazy_static! {
    pub static ref FOUNDATION_PK: MainPubkey = {
        let compile_time_key = option_env!("FOUNDATION_PK").unwrap_or(DEFAULT_FOUNDATION_PK_STR);
        let runtime_key =
            std::env::var("FOUNDATION_PK").unwrap_or_else(|_| compile_time_key.to_string());

        if runtime_key == DEFAULT_FOUNDATION_PK_STR {
            warn!("Using default FOUNDATION_PK: {}", DEFAULT_FOUNDATION_PK_STR);
        } else if runtime_key == compile_time_key {
            warn!("Using compile-time FOUNDATION_PK: {}", compile_time_key);
        } else {
            warn!("Overridden by runtime FOUNDATION_PK: {}", runtime_key);
        }

        match MainPubkey::from_hex(&runtime_key) {
            Ok(pk) => pk,
            Err(err) => panic!("Failed to parse foundation PK: {err:?}"),
        }
    };
}

lazy_static! {
    pub static ref NETWORK_ROYALTIES_PK: MainPubkey = {
        let compile_time_key =
            option_env!("NETWORK_ROYALTIES_PK").unwrap_or(DEFAULT_NETWORK_ROYALTIES_STR);
        let runtime_key =
            std::env::var("NETWORK_ROYALTIES_PK").unwrap_or_else(|_| compile_time_key.to_string());

        if runtime_key == DEFAULT_NETWORK_ROYALTIES_STR {
            warn!(
                "Using default NETWORK_ROYALTIES_PK: {}",
                DEFAULT_NETWORK_ROYALTIES_STR
            );
        } else if runtime_key == compile_time_key {
            warn!(
                "Using compile-time NETWORK_ROYALTIES_PK: {}",
                compile_time_key
            );
        } else {
            warn!(
                "Overridden by runtime NETWORK_ROYALTIES_PK: {}",
                runtime_key
            );
        }

        match MainPubkey::from_hex(&runtime_key) {
            Ok(pk) => pk,
            Err(err) => panic!("Failed to parse network royalties PK: {err:?}"),
        }
    };
    pub static ref DEFAULT_NETWORK_ROYALTIES_PK: MainPubkey = {
        match MainPubkey::from_hex(DEFAULT_NETWORK_ROYALTIES_STR) {
            Ok(pk) => pk,
            Err(err) => panic!("Failed to parse default network royalties PK: {err:?}"),
        }
    };
}

lazy_static! {
    pub static ref PAYMENT_FORWARD_PK: MainPubkey = {
        let compile_time_key =
            option_env!("PAYMENT_FORWARD_PK").unwrap_or(DEFAULT_PAYMENT_FORWARD_STR);
        let runtime_key =
            std::env::var("PAYMENT_FORWARD_PK").unwrap_or_else(|_| compile_time_key.to_string());

        if runtime_key == DEFAULT_PAYMENT_FORWARD_STR {
            warn!(
                "Using default PAYMENT_FORWARD_PK: {}",
                DEFAULT_PAYMENT_FORWARD_STR
            );
        } else if runtime_key == compile_time_key {
            warn!(
                "Using compile-time PAYMENT_FORWARD_PK: {}",
                compile_time_key
            );
        } else {
            warn!("Overridden by runtime PAYMENT_FORWARD_PK: {}", runtime_key);
        }

        match MainPubkey::from_hex(&runtime_key) {
            Ok(pk) => pk,
            Err(err) => panic!("Failed to parse payment forward PK: {err:?}"),
        }
    };
    pub static ref DEFAULT_PAYMENT_FORWARD_SK: SecretKey = {
        match SecretKey::from_hex(DEFAULT_PAYMENT_FORWARD_SK_STR) {
            Ok(sk) => sk,
            Err(err) => panic!("Failed to parse default payment forward SK: {err:?}"),
        }
    };
}

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
