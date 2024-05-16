// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::Client;

use futures::future::join_all;
use sn_transfers::{SpendAddress, WalletError, WalletResult};
use std::{collections::BTreeSet, iter::Iterator};
use tokio::time::Instant;

impl Client {
    /// Verify that a spend is valid on the network.
    /// Optionally verify its ancestors as well, all the way to genesis (might take a LONG time)
    ///
    /// Prints progress on stdout.
    ///
    /// When verifying all the way back to genesis, it only verifies Spends that are ancestors of the given Spend,
    /// ignoring all other branches.
    ///
    /// This is how the DAG it follows could look like:
    /// ```text
    ///              ... --
    ///                     \
    ///              ... ----                  ... --
    ///                       \                       \
    /// Spend0 -> Spend1 -----> Spend2 ---> Spend5 ---> Spend2 ---> Genesis
    ///                   \                           /
    ///                    ---> Spend3 ---> Spend6 ->
    ///                     \            /
    ///                      -> Spend4 ->
    ///                                /
    ///                            ...
    ///
    /// depth0    depth1        depth2      depth3      depth4      depth5
    /// ```
    ///
    /// This function will return an error if any spend in the way is invalid.
    pub async fn verify_spend_at(&self, addr: SpendAddress, to_genesis: bool) -> WalletResult<()> {
        let first_spend = self
            .get_spend_from_network(addr)
            .await
            .map_err(|err| WalletError::CouldNotVerifyTransfer(err.to_string()))?;

        if !to_genesis {
            return Ok(());
        }

        // use iteration instead of recursion to avoid stack overflow
        let mut txs_to_verify = BTreeSet::from_iter([first_spend.spend.parent_tx]);
        let mut depth = 0;
        let mut verified_tx = BTreeSet::new();
        let start = Instant::now();

        while !txs_to_verify.is_empty() {
            let mut next_gen_tx = BTreeSet::new();

            for parent_tx in txs_to_verify {
                let parent_tx_hash = parent_tx.hash();
                let parent_keys = parent_tx.inputs.iter().map(|input| input.unique_pubkey);
                let addrs_to_verify = parent_keys.map(|k| SpendAddress::from_unique_pubkey(&k));
                debug!("Depth {depth} - Verifying parent Tx : {parent_tx_hash:?} with inputs: {addrs_to_verify:?}");

                // get all parent spends in parallel
                let tasks: Vec<_> = addrs_to_verify
                    .clone()
                    .map(|a| self.get_spend_from_network(a))
                    .collect();
                let spends = join_all(tasks).await
                    .into_iter()
                    .zip(addrs_to_verify.into_iter())
                    .map(|(maybe_spend, a)|
                        maybe_spend.map_err(|err| WalletError::CouldNotVerifyTransfer(format!("at depth {depth} - Failed to get spend {a:?} from network for parent Tx {parent_tx_hash:?}: {err}"))))
                    .collect::<WalletResult<BTreeSet<_>>>()?;
                debug!(
                    "Depth {depth} - Got {:?} spends for parent Tx: {parent_tx_hash:?}",
                    spends.len()
                );
                trace!("Spends for {parent_tx_hash:?} - {spends:?}");

                // check if we reached the genesis Tx
                if parent_tx == *sn_transfers::GENESIS_CASHNOTE_PARENT_TX
                    && spends
                        .iter()
                        .all(|s| s.spend.unique_pubkey == *sn_transfers::GENESIS_SPEND_UNIQUE_KEY)
                    && spends.len() == 1
                {
                    debug!("Depth {depth} - Reached genesis Tx on one branch: {parent_tx_hash:?}");
                    verified_tx.insert(parent_tx_hash);
                    continue;
                }

                // verify tx with those spends
                parent_tx
                    .verify_against_inputs_spent(&spends)
                    .map_err(|err| WalletError::CouldNotVerifyTransfer(format!("at depth {depth} - Failed to verify parent Tx {parent_tx_hash:?}: {err}")))?;
                verified_tx.insert(parent_tx_hash);
                debug!("Depth {depth} - Verified parent Tx: {parent_tx_hash:?}");

                // add new parent spends to next gen
                next_gen_tx.extend(spends.into_iter().map(|s| s.spend.parent_tx));
            }

            // only verify parents we haven't already verified
            txs_to_verify = next_gen_tx
                .into_iter()
                .filter(|tx| !verified_tx.contains(&tx.hash()))
                .collect();

            depth += 1;
            let elapsed = start.elapsed();
            let n = verified_tx.len();
            println!("Now at depth {depth} - Verified {n} transactions in {elapsed:?}");
        }

        let elapsed = start.elapsed();
        let n = verified_tx.len();
        println!("Verified all the way to genesis! Through {depth} generations, verifying {n} transactions in {elapsed:?}");
        Ok(())
    }
}
