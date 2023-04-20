// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

/// Flow:
///  1. Client informs Nodes it wishes to spend a DBC, and attaches the DBC id (a `DbcId`).
///  2. Nodes individually calculate required fee, and returns that to Client.
///     The required fee consists of the `content` field, and `node_reward_key_sig` - the Node signature over it.
///     A `RequiredFeeContent` has the following fields:
///       a. `amount_cipher`: `RevealedAmount` (amount, blindfactor) ciphertext encrypted to id of DBC to spend (i.e. its `DbcId`).
///       b. `node_reward_key`:  Node's reward `PublicAddress`.
///  3. Client verifies Node's signature over `content`.
///  4. Client decrypts the `amount_cipher` to obtain the fee amount.
///  5. Client includes necessary DBC output in the intended spend, with the fee amount, deriving a new DBC id
///     using `node_reward_key` which is used in the DBC output to denote the new DBC destined to the Node.
///  6. Client then constructs the `FeeCiphers`to be included in the `Spend` request.
///     The FeeCiphers consists of the following fields:
///         a. `derivation_index_cipher`: The encrypted derivation index used to derive the new DBC id.
///         b. `amount_cipher`: The encrypted amount + blinding factor (`RevealedAmount`) which was used in the DBC output.
///  7. Client sends the `Spend` request to the Node.
///  9. Node verifies that:
///       a. the spend contains an output for them
///       b. the fee ciphers can be decrypted
///       c. the tx contains an output for a key derived from the Node `PublicAddress` using the decrypted derivation index
///       d. the amount in that output is the same as the decrypted amount
///       e. the decrypted amount is at most 1% less than the required fee at the time
///  10. Node is satisfied and stores the spend as a valid spend.
///
///      Note 1: The fee paid to the Node is not actually accessible until the Node can fetch all the `SignedSpend`s from
///         their respective close groups, and with that complete their DBC containing the fee.
///      Note 2: With 1 fee per spend, the fee amount in the dbc is not accessible to the Node until the required fee for
///         a spend has decreased below the amount in the dbc. In effect, with this design there is currently a lock-in
///         effect on Node rewards which require the network to grow for the amount paid to them to be accessible.
///         A directly accessible reward design has been discussed, where the reward payments have a unique tag that
///         identifies them and lets them be merged into a single DBC without paying fees. This is not yet implemented,
///         and it is not yet decided if it is even needed.
mod error;
mod fee_ciphers;
mod priority;
mod required_fee;
mod required_fee_content;
mod spend_queue;

pub use self::{
    error::{Error, Result},
    fee_ciphers::FeeCiphers,
    priority::SpendPriority,
    required_fee::RequiredFee,
    required_fee_content::RequiredFeeContent,
    spend_queue::{SpendQ, SpendQSnapshot, SpendQStats},
};

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;
    use sn_dbc::{MainKey, Token};

    #[test]
    #[allow(clippy::result_large_err)]
    fn required_fee_can_be_read_by_client() -> Result<()> {
        let main_key = MainKey::random();
        let derived_reward_key = main_key.random_derived_key(&mut rand::thread_rng());
        let dbc_id = derived_reward_key.dbc_id();

        let fee = Token::from_nano(1234);
        let required_fee = RequiredFee::new(fee, dbc_id, &main_key);

        // verify required fee is correctly signed
        let fee_sig_verification = required_fee.verify();
        assert_matches!(fee_sig_verification, Ok(()));

        // verify client can read the amount
        let decryption_result = required_fee.content.decrypt_amount(&derived_reward_key);
        assert_matches!(decryption_result, Ok(amount) => {
            assert_eq!(amount, fee);
        });

        Ok(())
    }
}
