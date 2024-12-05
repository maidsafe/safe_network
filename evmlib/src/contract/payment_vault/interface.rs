use crate::common::{Address, Amount, QuoteHash};
use alloy::sol;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    IPaymentVault,
    "abi/IPaymentVault.json"
);

impl From<(QuoteHash, Address, Amount)> for IPaymentVault::DataPayment {
    fn from(data: (QuoteHash, Address, Amount)) -> Self {
        Self {
            rewardsAddress: data.1,
            amount: data.2,
            quoteHash: data.0,
        }
    }
}
