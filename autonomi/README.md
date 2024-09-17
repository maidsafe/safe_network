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
