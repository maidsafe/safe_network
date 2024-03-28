// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use super::{data_payments::PaymentDetails, Result};
use crate::WalletError;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use xor_name::XorName;

const PAYMENTS_DIR_NAME: &str = "payments";
pub const WALLET_DIR_NAME: &str = "wallet";

/// Contains some common API's used by wallet implementations.
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct WalletApi {
    /// The dir of the wallet file, main key, public address, and new cash_notes.
    wallet_dir: Arc<PathBuf>,
    /// Cached version of `root_dir/wallet_dir/payments`
    payment_dir: Arc<PathBuf>,
}

impl WalletApi {
    /// Create a new instance give the root dir.
    pub fn new_from_root_dir(root_dir: &Path) -> Self {
        let wallet_dir = root_dir.join(WALLET_DIR_NAME);
        Self {
            payment_dir: Arc::new(wallet_dir.join(PAYMENTS_DIR_NAME)),
            wallet_dir: Arc::new(wallet_dir),
        }
    }

    /// Create a new instance give the root dir.
    pub fn new_from_wallet_dir(wallet_dir: &Path) -> Self {
        Self {
            wallet_dir: Arc::new(wallet_dir.to_path_buf()),
            payment_dir: Arc::new(wallet_dir.join(PAYMENTS_DIR_NAME)),
        }
    }

    /// Returns the most recent PaymentDetails for the given xorname if cached.
    /// If multiple payments have been made to the same xorname, then we pick the last one as it is the most recent.
    ///
    /// This does not check if the quote for the payment has expired.
    pub fn get_payment(&self, xorname: &XorName) -> Result<PaymentDetails> {
        let mut payments = self.read_payment_transactions(xorname)?;
        let payment = payments
            .pop()
            .ok_or(WalletError::NoPaymentForAddress(*xorname))?;
        info!("Payment retrieved for {xorname:?} from wallet");

        Ok(payment)
    }

    /// Returns the latest non-expired PaymentDetails for the given xorname.
    /// If multiple payments have been made to the same xorname, then we pick the last one which has not yet expired.
    ///
    /// If all the payments have expired, we return an error.
    pub fn get_non_expired_payment(&self, xorname: &XorName) -> Result<PaymentDetails> {
        let mut payment_details = self.get_all_payments(xorname)?;
        // find a non expired quote
        let payment_detail = loop {
            if let Some(payment_detail) = payment_details.pop() {
                if payment_detail.quote.has_expired() {
                    continue;
                } else {
                    break payment_detail;
                }
            } else {
                return Err(WalletError::QuoteExpired(*xorname));
            }
        };

        info!("Non-expired payment retrieved for {xorname:?} from wallet");

        Ok(payment_detail)
    }

    /// Return all the PaymentDetails for the given xorname if cached.
    /// Multiple payments to the same XorName can result in many payment details
    ///
    /// This does not check if the quote for the payments have expired.
    pub fn get_all_payments(&self, xorname: &XorName) -> Result<Vec<PaymentDetails>> {
        let payments = self.read_payment_transactions(xorname)?;
        if payments.is_empty() {
            return Err(WalletError::NoPaymentForAddress(*xorname));
        }
        info!(
            "All {} payments retrieved for {xorname:?} from wallet",
            payments.len()
        );

        Ok(payments)
    }

    /// Return all the non-expired PaymentDetails for the given xorname if cached.
    /// Multiple payments to the same xorname can result in many payment details
    ///
    /// If all the payments have expired, we return an error.
    pub fn get_all_non_expired_payments(&self, xorname: &XorName) -> Result<Vec<PaymentDetails>> {
        let payment_details = self.get_all_payments(xorname)?;

        let payments = payment_details
            .into_iter()
            .filter(|details| !details.quote.has_expired())
            .collect::<Vec<_>>();

        if payments.is_empty() {
            return Err(WalletError::QuoteExpired(*xorname));
        }

        info!(
            "All {} non-expired payments retrieved for {xorname:?} from wallet",
            payments.len()
        );

        Ok(payments)
    }

    /// Insert a payment and write it to the `payments` dir.
    /// If a prior payment has been made to the same xorname, then the new payment is pushed to the end of the list.
    pub fn insert_payment_transaction(&self, name: XorName, payment: PaymentDetails) -> Result<()> {
        // try to read the previous payments and push the new payment at the end
        let payments = match self.read_payment_transactions(&name) {
            Ok(mut stored_payments) => {
                stored_payments.push(payment);
                stored_payments
            }
            Err(_) => vec![payment],
        };
        let unique_file_name = format!("{}.payment", hex::encode(name));
        fs::create_dir_all(self.payment_dir.as_ref())?;

        let payment_file_path = self.payment_dir.join(unique_file_name);
        debug!("Writing payment to {payment_file_path:?}");

        let mut file = fs::File::create(payment_file_path)?;
        let mut serialiser = rmp_serde::encode::Serializer::new(&mut file);
        payments.serialize(&mut serialiser)?;
        Ok(())
    }

    pub fn remove_payment_transaction(&self, name: &XorName) {
        let unique_file_name = format!("{}.payment", hex::encode(*name));
        let payment_file_path = self.payment_dir.join(unique_file_name);

        debug!("Removing payment from {payment_file_path:?}");
        let _ = fs::remove_file(payment_file_path);
    }

    pub fn wallet_dir(&self) -> &Path {
        &self.wallet_dir
    }

    /// Read all the payments made to the provided xorname
    fn read_payment_transactions(&self, name: &XorName) -> Result<Vec<PaymentDetails>> {
        let unique_file_name = format!("{}.payment", hex::encode(*name));
        let payment_file_path = self.payment_dir.join(unique_file_name);

        debug!("Getting payment from {payment_file_path:?}");
        let file = fs::File::open(&payment_file_path)?;
        let payments = rmp_serde::from_read(&file)?;

        Ok(payments)
    }
}
