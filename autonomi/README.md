# `autonomi` - Autonomi client API

[![Crates.io](https://img.shields.io/crates/v/autonomi.svg)](https://crates.io/crates/autonomi)
[![docs.rs](https://img.shields.io/badge/api-rustdoc-blue.svg)](https://docs.rs/autonomi)

Connect to and build on the Autonomi network.

## Usage

See [docs.rs/autonomi](https://docs.rs/autonomi) for usage examples.

## Running tests

1. Run a local EVM node:

```sh
cargo run --bin evm_testnet -- --royalties-wallet <ETHEREUM_ADDRESS>
```

Take note of the console output for the next step (`RPC URL`, `Payment token address` & `Chunk payments address`).

2. Run a local network with the `local-discovery` feature and pass the EVM params:

```sh
cargo run --bin=safenode-manager --features=local-discovery -- local run --build --clean --rewards-address <ETHEREUM_ADDRESS> evm-custom --rpc-url <RPC_URL> --payment-token-address <TOKEN_ADDRESS> --chunk-payments-address <CONTRACT_ADDRESS>
```

3. Then run the tests with the `local` feature and pass the EVM params again:

```sh
$ RPC_URL=<RPC_URL> PAYMENT_TOKEN_ADDRESS=<TOKEN_ADDRESS> CHUNK_PAYMENTS_ADDRESS=<CONTRACT_ADDRESS> cargo test --package=autonomi --features=local
# Or with logs
$ RUST_LOG=autonomi RPC_URL=<RPC_URL> PAYMENT_TOKEN_ADDRESS=<TOKEN_ADDRESS> CHUNK_PAYMENTS_ADDRESS=<CONTRACT_ADDRESS> cargo test --package=autonomi --features=local -- --nocapture
```

## Faucet (local)

There is no faucet server, but instead you can use the `Deployer wallet private key` printed in the EVM node output to
initialise a wallet from with almost infinite gas and payment tokens. Example:

```rust
let private_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
let network = evm_network_from_env();
let deployer_wallet = Wallet::new_from_private_key(network, private_key).unwrap();
let receiving_wallet = Wallet::new_with_random_wallet(network);

// Send 10 payment tokens (atto)
let _ = deployer_wallet
.transfer_tokens(receiving_wallet.address(), Amount::from(10))
.await;
```


