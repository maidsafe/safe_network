// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use sn_dbc::{
    rng, Dbc, DbcId, DbcIdSource, DbcTransaction, DerivedKey, Error as DbcError, Hash,
    InputHistory, MainKey, PublicAddress, RevealedAmount, RevealedInput, Token, TransactionBuilder,
};

use std::{collections::BTreeSet, fmt::Debug, result};
use thiserror::Error;

/// Amount of tokens to be owned by the Genesis DBC.
/// At the inception of the Network a total supply of 4,294,967,295 whole tokens will be created.
/// Each whole token can be subdivided 10^9 times,
/// thus creating a total of 4,294,967,295,000,000,000 available units.
pub const GENESIS_DBC_AMOUNT: u64 = u32::MAX as u64 * u64::pow(10, 9);

/// A specialised `Result` type for types crate.
pub type GenesisResult<T> = result::Result<T, GenesisError>;

/// Main error type for the crate.
#[derive(Error, Debug, Clone)]
// #[non_exhaustive]
#[non_exhaustive]
pub enum GenesisError {
    /// Error occurred when minting the Genesis DBC.
    #[error("Genesis DBC error:: {0}")]
    GenesisDbcError(String),
    /// FailedToParseReason
    #[error("Failed to parse reason: {0}")]
    FailedToParseReason(#[from] DbcError),
}

/// Generate the genesis DBC.
///
/// Requires the initial section key to sign the share and a different secret key for the DBC.
///
/// The genesis DBC will be created using a different key from the initial section key. This is
/// because the genesis DBC, along with its secret key, will be publicly available for auditing
/// purposes. It needs to be a set rather than just a key because a spent proof share gets
/// generated, which requires a key set. We can't use the same key for the genesis DBC and section
/// because if the genesis DBC is publicly available, the secret key could be used to create a bad
/// section that would be accepted by the network.
#[allow(clippy::result_large_err)]
pub fn create_genesis_dbc(genesis_main_key: &MainKey) -> GenesisResult<Dbc> {
    let rng = &mut rng::thread_rng();

    let dbc_id_src = genesis_main_key.random_dbc_id_src(rng);
    let derived_key = genesis_main_key.derive_key(&dbc_id_src.derivation_index);
    let revealed_amount = RevealedAmount::from_amount(GENESIS_DBC_AMOUNT, rng);

    // Use the same key as the input and output of Genesis Tx.
    // The src tx is empty as this is the first DBC.
    let genesis_input = InputHistory {
        input: RevealedInput::new(derived_key, revealed_amount),
        input_src_tx: DbcTransaction {
            inputs: vec![],
            outputs: vec![],
        },
    };

    let dbc_builder = TransactionBuilder::default()
        .add_input(genesis_input)
        .add_output(Token::from_nano(GENESIS_DBC_AMOUNT), dbc_id_src)
        .build(Hash::default(), rng::thread_rng())
        .map_err(|err| {
            GenesisError::GenesisDbcError(format!(
                "Failed to build the DBC transaction for genesis DBC: {err}",
            ))
        })?;

    // build the output DBCs
    let output_dbcs = dbc_builder.build_without_verifying().map_err(|err| {
        GenesisError::GenesisDbcError(format!(
            "DBC builder failed to create output genesis DBC: {err}",
        ))
    })?;

    // just one output DBC is expected which is the genesis DBC
    let (genesis_dbc, _) = output_dbcs.into_iter().next().ok_or_else(|| {
        GenesisError::GenesisDbcError(
            "DBC builder (unexpectedly) contains an empty set of outputs.".to_string(),
        )
    })?;

    Ok(genesis_dbc)
}

// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

pub(super) type Result<T> = std::result::Result<T, Error>;

/// Error type returned by the API
#[derive(Debug, Error)]
#[allow(clippy::large_enum_variant)]
#[non_exhaustive]
pub enum Error {
    /// Not enough balance to perform a transaction
    #[error("Not enough balance: {0}")]
    NotEnoughBalance(String),
    /// Not enough was paid in fees for the Elders to process the spend.
    #[error("Too low amount for the transfer. Highest required fee: {0:?}.")]
    FeeTooLow(Token),
    /// DbcError
    #[error("DbcError: {0}")]
    DbcError(#[from] DbcError),
    /// DbcReissueError
    #[error("DbcReissueError: {0}")]
    DbcReissueError(String),
    /// Verification of DBC validly signed by a known section failed
    #[error("DBC validity verification failed: {0}")]
    DbcVerificationFailed(String),
}

/// Send the tokens to the specified destination keys, using the provided dbcs.
/// The new dbcs that are created, one per specified destination, will have the
/// unique id which is the public key of the `OwnerOnce` instances provided.
///
/// Transfer fees will be paid if not in data-network.
/// The input dbcs will be spent on the network, and the resulting
/// dbcs (and change dbc if any) are returned.
/// NB: We are skipping the DbcReason arg for now. It can be added later.
#[allow(clippy::result_large_err)]
pub fn send_tokens(
    dbcs: Vec<(Dbc, DerivedKey)>,
    recipients: Vec<(Token, DbcIdSource)>,
    change_to: PublicAddress,
    // reason: DbcReason,
    // priority: SpendPriority,
) -> Result<ReissueOutputs> {
    // let mut attempts = 0;
    // let mut max_required = Token::zero();

    // We need to select the necessary number of dbcs from those that we were passed.
    // This will also account for any fees.
    let reissue_inputs = select_inputs(dbcs, recipients, change_to)?;

    reissue(reissue_inputs)
    // // then we can reissue
    // match reissue(reissue_inputs).await {
    //     Ok(outputs) => return Ok(outputs),
    //     Err(Error::FeeTooLow(required)) => {
    //         max_required = required;
    //         attempts += 1;
    //     }
    //     error => return error,
    // }

    // Err(Error::FeeTooLow(max_required))
}

/// The input dbcs will be spent on the network, and the resulting
/// dbcs (and change dbc if any) are returned.
/// This will pay transfer fees if not in data-network.
#[allow(clippy::result_large_err)]
fn reissue(
    // client: &Client,
    reissue_inputs: ReissueInputs,
    // reason: DbcReason,
) -> Result<ReissueOutputs> {
    let ReissueInputs {
        input_dbcs,
        outputs,
        change: (change, change_to),
        // #[cfg(not(feature = "data-network"))]
        // fees_paid,
        // #[cfg(not(feature = "data-network"))]
        // all_fee_cipher_params,
    } = reissue_inputs;

    let mut inputs = vec![];
    for (dbc, derived_key) in input_dbcs {
        let revealed_amount = match dbc.revealed_amount(&derived_key) {
            Ok(amount) => amount,
            Err(err) => {
                warn!("Ignoring dbc, as it didn't have the correct derived key: {err}");
                continue;
            }
        };
        let input = InputHistory {
            input: RevealedInput::new(derived_key, revealed_amount),
            input_src_tx: dbc.src_tx,
        };
        inputs.push(input);
    }

    let mut tx_builder = TransactionBuilder::default()
        .add_inputs(inputs)
        .add_outputs(outputs);

    let mut rng = rng::thread_rng();

    let dbc_id_src = change_to.random_dbc_id_src(&mut rng);
    if change.as_nano() > 0 {
        tx_builder = tx_builder.add_output(change, dbc_id_src);
    }

    // let inputs_spent_proofs: BTreeSet<SpentProof> = input_dbcs
    //     .iter()
    //     .flat_map(|dbc| dbc.inputs_spent_proofs.clone())
    //     .collect();
    // let inputs_spent_transactions: BTreeSet<DbcTransaction> = input_dbcs
    //     .iter()
    //     .flat_map(|dbc| dbc.inputs_spent_transactions.clone())
    //     .collect();

    // Finalize the tx builder to get the dbc builder.
    let dbc_builder = tx_builder
        .build(Hash::default(), &mut rng)
        .map_err(Error::DbcError)?;

    // // Get fee outputs to generate the fee ciphers.
    // #[cfg(not(feature = "data-network"))]
    // let outputs = dbc_builder
    //     .revealed_outputs
    //     .iter()
    //     .map(|output| (output.public_key, output.revealed_amount))
    //     .collect();

    let inputs = dbc_builder.signed_spends();
    let spent_dbcs = inputs.iter().map(|spent| spent.dbc_id()).cloned().collect();

    // // Spend all the input DBCs, collecting the spent proof shares for each of them
    // for (public_key, tx) in inputs {
    //     // // Generate the fee ciphers.
    //     // #[cfg(not(feature = "data-network"))]
    //     // let input_fee_ciphers = {
    //     //     let fee_cipher_params = all_fee_cipher_params
    //     //         .get(&public_key)
    //     //         .ok_or(Error::DbcReissueError("Missing fee!".to_string()))?;
    //     //     fee_ciphers(&outputs, fee_cipher_params)?
    //     // };

    //     let tx_hash = Hash::from(tx.hash());
    //     // TODO: spend DBCs concurrently spawning tasks
    //     let mut attempts = 0;
    //     loop {
    //         attempts += 1;
    //         client
    //             .spend_dbc(
    //                 public_key,
    //                 tx.clone(),
    //                 reason,
    //                 inputs_spent_proofs.clone(),
    //                 inputs_spent_transactions.clone(),
    //                 #[cfg(not(feature = "data-network"))]
    //                 input_fee_ciphers.clone(),
    //             )
    //             .await?;

    //         let signed_spend = client.get_signed_spend(public_key).await?;
    //     }
    // }

    // Perform verifications of input TX and spentproofs,
    // as well as building the output DBCs.
    let mut output_dbcs = dbc_builder.build().map_err(Error::DbcError)?;

    let mut change_dbc = None;
    output_dbcs.retain(|(dbc, _)| {
        if dbc.public_address() == &change_to && change.as_nano() > 0 {
            change_dbc = Some(dbc.clone());
            false
        } else {
            true
        }
    });

    Ok(ReissueOutputs {
        outputs: output_dbcs,
        change: change_dbc,
        spent_dbcs,
        // #[cfg(not(feature = "data-network"))]
        // fees_paid,
    })
}

///
#[allow(clippy::result_large_err)]
pub(crate) fn select_inputs(
    inputs: Vec<(Dbc, DerivedKey)>,
    outputs: Vec<(Token, DbcIdSource)>,
    change_to: PublicAddress,
) -> Result<ReissueInputs> {
    // We'll combine one or more input DBCs and reissue:
    // - one output DBC per recipient,
    // - and a single DBC for the change - if any - which will be returned from this function.
    let mut input_dbcs = Vec::new();
    let mut total_input_amount = Token::zero();
    let total_output_amount = outputs
        .iter()
        .fold(Some(Token::zero()), |total, (amount, _)| {
            total.and_then(|t| t.checked_add(*amount))
        })
        .ok_or_else(|| {
            Error::DbcReissueError(
                "Overflow occurred while summing the output amounts for the output DBCs."
                    .to_string(),
            )
        })?;

    let mut change_amount = total_output_amount;
    // let mut rng = rng::thread_rng();
    // #[cfg(not(feature = "data-network"))]
    // let mut all_fee_cipher_params = BTreeMap::new();
    // #[cfg(not(feature = "data-network"))]
    // let mut fees_paid = Token::zero();

    for (dbc, derived_key) in inputs {
        // let revealed_amount = match dbc.revealed_amount(derived_key).map_err(Error::DbcError) {
        //     Ok(revealed) => revealed,
        //     Err(error) => {
        //         error!(
        //             "Could not get the amount from dbc {:?}, wrong derived key used! {error}",
        //             dbc.id(),
        //         );
        //         continue;
        //     }
        // };

        let input_key = dbc.id();

        #[cfg(not(feature = "data-network"))]
        {
            // // ------------ fee part start ----------------
            // #[cfg(not(feature = "data-network"))]
            // let fee_per_input = {
            //     // Each section will have elder_count() instances to pay individually (for now, later they will be more).
            //     let elder_fees = match client.get_section_fees(input_key, priority).await {
            //         Ok(fees) => fees,
            //         Err(error) => {
            //             error!("Could not get fees for input dbc: {input_key:?}: {error}");
            //             continue;
            //         }
            //     };
            //     let num_responses = elder_fees.len();
            //     let required_responses = supermajority(elder_count());
            //     if required_responses > num_responses {
            //         warn!("Not enough elders contacted for the section to spend the input. Got: {num_responses}, needed: {required_responses}");
            //         continue;
            //     }

            //     // Fees that were not encrypted to us.
            //     let mut invalid_fees = BTreeSet::new();
            //     // As the Elders encrypt the amount to our public key, we need to decrypt it.
            //     let mut decrypted_elder_fees = vec![];

            //     for (elder, fee) in elder_fees {
            //         match fee.content.decrypt_amount(&revealed_bearer.secret_key) {
            //             Ok(amount) => decrypted_elder_fees.push(((elder, fee), amount)),
            //             Err(error) => {
            //                 error!("Decrypting the fee content from {elder} failed! {error}");
            //                 let _ = invalid_fees.insert(fee.content.elder_reward_key);
            //             }
            //         }
            //     }

            //     let max_invalid_fees = elder_count() - required_responses;
            //     if invalid_fees.len() > max_invalid_fees {
            //         let valid_responses = num_responses - invalid_fees.len();
            //         warn!("Not enough valid fees received from the section to spend the input. Found: {valid_responses}, needed: {required_responses}", );
            //         continue;
            //     }

            //     // Total fee paid to all recipients in the section for this input.
            //     let fee_per_input = decrypted_elder_fees
            //         .iter()
            //         .fold(Some(Token::zero()), |total, (_, fee)| {
            //             total.and_then(|t| t.checked_add(*fee))
            //         })
            //         .ok_or_else(|| Error::DbcReissueError(
            //             "Overflow occurred while summing the individual Elder's fees in order to calculate the total amount for the output DBCs."
            //                 .to_string(),
            //         ))?;

            //     let mut fee_cipher_params = BTreeMap::new();

            //     // Add elders to outputs and generate their fee ciphers.
            //     decrypted_elder_fees
            //         .iter()
            //         .for_each(|((elder, required_fee), fee)| {
            //             let owner = Owner::from(required_fee.content.elder_reward_key);
            //             let owner_once = OwnerOnce::from_owner_base(owner, &mut rng);
            //             outputs.push((*fee, owner_once.clone()));

            //             let _ =
            //                 fee_cipher_params.insert(elder.name(), (required_fee.clone(), owner_once));
            //         });

            //     let _ = all_fee_cipher_params.insert(input_key, fee_cipher_params);

            //     fees_paid = fees_paid.checked_add(fee_per_input).ok_or_else(|| {
            //         Error::DbcReissueError(
            //             "Overflow occurred while summing all the input fees.".to_string(),
            //         )
            //     })?;

            //     fee_per_input
            // };
            // // ---------------- fee part end ----------------
        }

        let dbc_balance = match dbc.revealed_amount(&derived_key) {
            Ok(revealed_amount) => Token::from_nano(revealed_amount.value()),
            Err(err) => {
                warn!("Ignoring input Dbc (id: {input_key:?}) due to not having correct derived key: {err:?}");
                continue;
            }
        };

        // Add this Dbc as input to be spent.
        input_dbcs.push((dbc, derived_key));

        // Input amount increases with the amount of the dbc.
        total_input_amount = total_input_amount.checked_add(dbc_balance)
            .ok_or_else(|| {
                Error::DbcReissueError(
                    "Overflow occurred while increasing total input amount while trying to cover the output DBCs."
                    .to_string(),
            )
            })?;

        #[cfg(not(feature = "data-network"))]
        {
            // // Output amount now increases a bit, as we have to cover the fee as well..
            // total_output_amount = total_output_amount.checked_add(fee_per_input)
            // .ok_or_else(|| {
            //     Error::DbcReissueError(
            //     "Overflow occurred while adding mint fee in order to calculate the total amount for the output DBCs."
            //         .to_string(),
            // )
            // })?;
            // // ..and so does `change_amount` (that we subtract from to know if we've covered `total_output_amount`).
            // change_amount = change_amount.checked_add(fee_per_input)
            // .ok_or_else(|| {
            //     Error::DbcReissueError(
            //     "Overflow occurred while adding mint fee in order to calculate the total amount for the output DBCs."
            //         .to_string(),
            // )
            // })?;
        }

        // If we've already combined input DBCs for the total output amount, then stop.
        match change_amount.checked_sub(dbc_balance) {
            Some(pending_output) => {
                change_amount = pending_output;
                if change_amount.as_nano() == 0 {
                    break;
                }
            }
            None => {
                change_amount = Token::from_nano(dbc_balance.as_nano() - change_amount.as_nano());
                break;
            }
        }
    }

    // If not enough spendable was found, this check will return an error.
    verify_amounts(total_input_amount, total_output_amount)?;

    Ok(ReissueInputs {
        input_dbcs,
        outputs,
        change: (change_amount, change_to),
        // #[cfg(not(feature = "data-network"))]
        // fees_paid,
        // #[cfg(not(feature = "data-network"))]
        // all_fee_cipher_params,
    })
}

// Make sure total input amount gathered with input DBCs are enough for the output amount
#[allow(clippy::result_large_err)]
fn verify_amounts(total_input_amount: Token, total_output_amount: Token) -> Result<()> {
    if total_output_amount > total_input_amount {
        return Err(Error::NotEnoughBalance(total_input_amount.to_string()));
    }
    Ok(())
}

// type ReissueCiphers = BTreeMap<PublicKey, BTreeMap<XorName, (RequiredFee, OwnerOnce)>>;

///
#[derive(Debug)]
pub struct ReissueInputs {
    /// The dbcs to spend, as to send tokens.
    pub input_dbcs: Vec<(Dbc, DerivedKey)>,
    /// The dbcs that will be created, holding the tokens to send.
    pub outputs: Vec<(Token, DbcIdSource)>,
    /// Any surplus amount after spending the necessary input dbcs.
    pub change: (Token, PublicAddress),
    // /// Total fees to pay for spending the inputs.
    // #[cfg(not(feature = "data-network"))]
    // pub fees_paid: Token,
    // /// This is the set of input dbc keys, each having a set of
    // /// Elder names and their respective fee ciphers.
    // /// Sent together with spends, so that Elders can verify their fee payments.
    // #[cfg(not(feature = "data-network"))]
    // pub all_fee_cipher_params: ReissueCiphers,
}

/// The results of reissuing dbcs.
#[derive(Debug)]
pub struct ReissueOutputs {
    /// The dbcs holding the tokens that were sent.
    pub outputs: Vec<(Dbc, RevealedAmount)>,
    /// The dbc holding surplus amount after spending the necessary input dbcs.
    pub change: Option<Dbc>,
    /// The dbcs we spent when reissuing.
    pub spent_dbcs: BTreeSet<DbcId>,
    // /// Total fees to paid for this tx.
    // #[cfg(not(feature = "data-network"))]
    // pub fees_paid: Token,
}
