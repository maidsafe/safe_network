
 # Arguments

 * `client.clone()` - A clone of the struct: `sn_client::Client`
 * `wallet` - The struct `sn_transfers::LocalWallet`

 # Example
 ```ignore
 let mut wallet_client = WalletClient::new(client.clone(), wallet);
 ```