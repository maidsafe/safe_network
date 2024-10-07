# `autonomi` - Autonomi client API

[![Crates.io](https://img.shields.io/crates/v/autonomi.svg)](https://crates.io/crates/autonomi)
[![docs.rs](https://img.shields.io/badge/api-rustdoc-blue.svg)](https://docs.rs/autonomi)

Connect to and build on the Autonomi network.

## Usage

Add the autonomi crate to your `Cargo.toml`:

```toml
[dependencies]
autonomi = { path = "../autonomi", version = "0.1.0" }
```

## Running tests

### Using a local EVM testnet

1. If you haven't, install Foundry, to be able to run Anvil
   nodes: https://book.getfoundry.sh/getting-started/installation
2. Run a local EVM node:

```sh
cargo run --bin evm_testnet
```

3. Run a local network with the `local-discovery` feature and use the local evm node. 

```sh
cargo run --bin=safenode-manager --features=local-discovery -- local run --build --clean --rewards-address <ETHEREUM_ADDRESS> evm-local
```

4. Then run the tests with the `local` feature and pass the EVM params again:

```sh
$ EVM_NETWORK=local cargo test --package=autonomi --features=local
# Or with logs
$ RUST_LOG=autonomi EVM_NETWORK=local cargo test --package=autonomi --features=local -- --nocapture
```

### Using a live testnet or mainnet

Using the hardcoded `Arbitrum One` option as an example, but you can also use the command flags of the steps above and
point it to a live network.

1. Run a local network with the `local-discovery` feature:

```sh
cargo run --bin=safenode-manager --features=local-discovery -- local run --build --clean --rewards-address <ETHEREUM_ADDRESS> evm-arbitrum-one
```

2. Then run the tests with the `local` feature. Make sure that the wallet of the private key you pass has enough gas and
   payment tokens on the network (in this case Arbitrum One):

```sh
$ EVM_NETWORK=arbitrum-one EVM_PRIVATE_KEY=<PRIVATE_KEY> cargo test --package=autonomi --features=local
# Or with logs
$ RUST_LOG=autonomi EVM_NETWORK=arbitrum-one EVM_PRIVATE_KEY=<PRIVATE_KEY> cargo test --package=autonomi --features=local -- --nocapture
```

### WebAssembly

To run a WASM test
- Install `wasm-pack`
- Make sure your Rust supports the `wasm32-unknown-unknown` target. (If you have `rustup`: `rustup target add wasm32-unknown-unknown`.)
- Pass a bootstrap peer via `SAFE_PEERS`. This *has* to be the websocket address, e.g. `/ip4/<ip>/tcp/<port>/ws/p2p/<peer ID>`.
    - As well as the other environment variables needed for EVM payments (e.g. `RPC_URL`).
- Optionally specify the specific test, e.g. `-- put` to run `put()` in `wasm.rs` only.

Example:
````sh
SAFE_PEERS=/ip4/<ip>/tcp/<port>/ws/p2p/<peer ID> wasm-pack test --release --firefox autonomi --features=data,files --test wasm -- put
```


## Faucet (local)

There is no faucet server, but instead you can use the `Deployer wallet private key` printed in the EVM node output to
initialise a wallet from with almost infinite gas and payment tokens. Example:

```rust
let rpc_url = "http://localhost:54370/";
let payment_token_address = "0x5FbDB2315678afecb367f032d93F642f64180aa3";
let data_payments_address = "0x8464135c8F25Da09e49BC8782676a84730C318bC";
let private_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

let network = Network::Custom(CustomNetwork::new(
rpc_url,
payment_token_address,
data_payments_address,
));

let deployer_wallet = Wallet::new_from_private_key(network, private_key).unwrap();
let receiving_wallet = Wallet::new_with_random_wallet(network);

// Send 10 payment tokens (atto)
let _ = deployer_wallet
.transfer_tokens(receiving_wallet.address(), Amount::from(10))
.await;
```

Alternatively, you can provide the wallet address that should own all the gas and payment tokens to the EVM testnet
startup command using the `--genesis-wallet` flag:

```sh
cargo run --bin evm_testnet -- --genesis-wallet <ETHEREUM_ADDRESS>
```

```shell
*************************
* Ethereum node started *
*************************
RPC URL: http://localhost:60093/
Payment token address: 0x5FbDB2315678afecb367f032d93F642f64180aa3
Chunk payments address: 0x8464135c8F25Da09e49BC8782676a84730C318bC
Deployer wallet private key: 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
Genesis wallet balance: (tokens: 20000000000000000000000000, gas: 9998998011366954730202)
```