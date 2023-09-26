// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#[macro_use]
extern crate tracing;

mod address;
mod builder;
mod cashnote;
mod error;
mod fee_output;
mod genesis;
mod nano;
mod reason_hash;
mod signed_spend;
mod transaction;
mod transfers;
mod unique_keys;
mod wallet;

pub(crate) use crate::{builder::TransactionBuilder, fee_output::FeeOutput, transaction::Input};

/// Types used in the public API
pub use crate::{
    address::SpendAddress,
    cashnote::CashNote,
    error::{Error, Result},
    nano::NanoTokens,
    reason_hash::Hash,
    signed_spend::{SignedSpend, Spend},
    transaction::Transaction,
    transfers::{OfflineTransfer, SpendRequest},
    unique_keys::{DerivationIndex, DerivedSecretKey, MainPubkey, MainSecretKey, UniquePubkey},
    wallet::Transfer,
};

/// Utilities exposed
pub use crate::{
    genesis::{
        create_faucet_wallet, create_first_cash_note_from_key, is_genesis_parent_tx,
        load_genesis_wallet,
    },
    genesis::{Error as GenesisError, GENESIS_CASHNOTE},
    transfers::create_offline_transfer,
    wallet::{bls_secret_from_hex, parse_main_pubkey},
    wallet::{Error as WalletError, LocalWallet, Result as WalletResult},
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

    pub fn thread_rng() -> ThreadRng {
        crate::rand::thread_rng()
    }

    pub fn from_seed(seed: <StdRng as SeedableRng>::Seed) -> StdRng {
        StdRng::from_seed(seed)
    }
}
