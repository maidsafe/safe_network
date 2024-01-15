# Arguments

* `client.clone()` - A cloned instance of the struct: `sn_client::Client`
* `wallet` - An instance of the struct `sn_transfers::LocalWallet`

# Example

 ```ignore
 let mut wallet_client = WalletClient::new(client.clone(), wallet);
 ```