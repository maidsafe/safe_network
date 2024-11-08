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

3. Run a local network with the `local` feature and use the local evm node.

```sh
cargo run --bin=safenode-manager --features=local -- local run --build --clean --rewards-address <ETHEREUM_ADDRESS> evm-local
```

4. Then run the tests with the `local` feature and pass the EVM params again:

```sh
EVM_NETWORK=local cargo test --package=autonomi --features=local
# Or with logs
RUST_LOG=autonomi EVM_NETWORK=local cargo test --package=autonomi --features=local -- --nocapture
```

### Using a live testnet or mainnet

Using the hardcoded `Arbitrum One` option as an example, but you can also use the command flags of the steps above and
point it to a live network.

1. Run a local network with the `local` feature:

```sh
cargo run --bin=safenode-manager --features=local -- local run --build --clean --rewards-address <ETHEREUM_ADDRESS> evm-arbitrum-one
```

2. Then run the tests with the `local` feature. Make sure that the wallet of the private key you pass has enough gas and
   payment tokens on the network (in this case Arbitrum One):

```sh
EVM_NETWORK=arbitrum-one EVM_PRIVATE_KEY=<PRIVATE_KEY> cargo test --package=autonomi --features=local
# Or with logs
RUST_LOG=autonomi EVM_NETWORK=arbitrum-one EVM_PRIVATE_KEY=<PRIVATE_KEY> cargo test --package=autonomi --features=local -- --nocapture
```

### WebAssembly

To run a WASM test

- Install `wasm-pack`
- Make sure your Rust supports the `wasm32-unknown-unknown` target. (If you
  have `rustup`: `rustup target add wasm32-unknown-unknown`.)
- Pass a bootstrap peer via `SAFE_PEERS`. This *has* to be the websocket address,
  e.g. `/ip4/<ip>/tcp/<port>/ws/p2p/<peer ID>`.
    - As well as the other environment variables needed for EVM payments (e.g. `RPC_URL`).
- Optionally specify the specific test, e.g. `-- put` to run `put()` in `wasm.rs` only.

Example:

```sh
SAFE_PEERS=/ip4/<ip>/tcp/<port>/ws/p2p/<peer ID> wasm-pack test --release --firefox autonomi --features=data,files --test wasm -- put
```

#### Test from JS in the browser

`wasm-pack test` does not execute JavaScript, but runs mostly WebAssembly. Again make sure the environment variables are
set and build the JS package:

```sh
wasm-pack build --dev --target=web autonomi --features=vault
```

Then cd into `autonomi/tests-js`, and use `npm` to install and serve the test html file.

```
cd autonomi/tests-js
npm install
npm run serve
```

Then go to `http://127.0.0.1:8080/tests-js` in the browser. Here, enter a `ws` multiaddr of a local node and press '
run'.

#### MetaMask example

There is a MetaMask example for doing a simple put operation.

Build the package with the `external-signer` feature (and again with the env variables) and run a webserver, e.g. with
Python:

```sh
wasm-pack build --dev --target=web autonomi --features=external-signer
python -m http.server --directory=autonomi 8000
```

Then visit `http://127.0.0.1:8000/examples/metamask` in your (modern) browser.

Here, enter a `ws` multiaddr of a local node and press 'run'.

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

## Python Bindings

The Autonomi client library provides Python bindings for easy integration with Python applications.

### Installation

```bash
pip install autonomi-client
```

### Quick Start

```python
from autonomi_client import Client, Wallet, PaymentOption

# Initialize wallet with private key
wallet = Wallet("your_private_key_here")
print(f"Wallet address: {wallet.address()}")
print(f"Balance: {wallet.balance()}")

# Connect to network
client = Client.connect(["/ip4/127.0.0.1/tcp/12000"])

# Create payment option
payment = PaymentOption.wallet(wallet)

# Upload data
data = b"Hello, Safe Network!"
addr = client.data_put(data, payment)
print(f"Data uploaded to: {addr}")

# Download data
retrieved = client.data_get(addr)
print(f"Retrieved: {retrieved.decode()}")
```

### Available Modules

#### Core Components

- `Client`: Main interface to the Autonomi network
  - `connect(peers: List[str])`: Connect to network nodes
  - `data_put(data: bytes, payment: PaymentOption)`: Upload data
  - `data_get(addr: str)`: Download data
  - `private_data_put(data: bytes, payment: PaymentOption)`: Store private data
  - `private_data_get(access: PrivateDataAccess)`: Retrieve private data
  - `register_generate_key()`: Generate register key

- `Wallet`: Ethereum wallet management
  - `new(private_key: str)`: Create wallet from private key
  - `address()`: Get wallet address
  - `balance()`: Get current balance

- `PaymentOption`: Payment configuration
  - `wallet(wallet: Wallet)`: Create payment option from wallet

#### Private Data

- `PrivateDataAccess`: Handle private data storage
  - `from_hex(hex: str)`: Create from hex string
  - `to_hex()`: Convert to hex string
  - `address()`: Get short reference address

```python
# Private data example
access = client.private_data_put(secret_data, payment)
print(f"Private data stored at: {access.to_hex()}")
retrieved = client.private_data_get(access)
```

#### Registers

- Register operations for mutable data
  - `register_create(value: bytes, name: str, key: RegisterSecretKey, wallet: Wallet)`
  - `register_get(address: str)`
  - `register_update(register: Register, value: bytes, key: RegisterSecretKey)`

```python
# Register example
key = client.register_generate_key()
register = client.register_create(b"Initial value", "my_register", key, wallet)
client.register_update(register, b"New value", key)
```

#### Vaults

- `VaultSecretKey`: Manage vault access
  - `new()`: Generate new key
  - `from_hex(hex: str)`: Create from hex string
  - `to_hex()`: Convert to hex string

- `UserData`: User data management
  - `new()`: Create new user data
  - `add_file_archive(archive: str)`: Add file archive
  - `add_private_file_archive(archive: str)`: Add private archive
  - `file_archives()`: List archives
  - `private_file_archives()`: List private archives

```python
# Vault example
vault_key = VaultSecretKey.new()
cost = client.vault_cost(vault_key)
client.write_bytes_to_vault(data, payment, vault_key, content_type=1)
data, content_type = client.fetch_and_decrypt_vault(vault_key)
```

#### Utility Functions

- `encrypt(data: bytes)`: Self-encrypt data
- `hash_to_short_string(input: str)`: Generate short reference

### Complete Examples

#### Data Management

```python
def handle_data_operations(client, payment):
    # Upload text
    text_data = b"Hello, Safe Network!"
    text_addr = client.data_put(text_data, payment)
    
    # Upload binary data
    with open("image.jpg", "rb") as f:
        image_data = f.read()
        image_addr = client.data_put(image_data, payment)
    
    # Download and verify
    downloaded = client.data_get(text_addr)
    assert downloaded == text_data
```

#### Private Data and Encryption

```python
def handle_private_data(client, payment):
    # Create and encrypt private data
    secret = {"api_key": "secret_key"}
    data = json.dumps(secret).encode()
    
    # Store privately
    access = client.private_data_put(data, payment)
    print(f"Access token: {access.to_hex()}")
    
    # Retrieve
    retrieved = client.private_data_get(access)
    secret = json.loads(retrieved.decode())
```

#### Vault Management

```python
def handle_vault(client, payment):
    # Create vault
    vault_key = VaultSecretKey.new()
    
    # Store user data
    user_data = UserData()
    user_data.add_file_archive("archive_address")
    
    # Save to vault
    cost = client.put_user_data_to_vault(vault_key, payment, user_data)
    
    # Retrieve
    retrieved = client.get_user_data_from_vault(vault_key)
    archives = retrieved.file_archives()
```

### Error Handling

All operations can raise exceptions. It's recommended to use try-except blocks:

```python
try:
    client = Client.connect(peers)
    # ... operations ...
except Exception as e:
    print(f"Error: {e}")
```

### Best Practices

1. Always keep private keys secure
2. Use error handling for all network operations
3. Clean up resources when done
4. Monitor wallet balance for payments
5. Use appropriate content types for vault storage

For more examples, see the `examples/` directory in the repository.
