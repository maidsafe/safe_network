# `sn_client` - SAFE Network Client Library

## Overview

The `sn_client` library provides the core functionalities for interacting with the SAFE Network. It handles tasks such as connecting to the network, managing concurrency, and performing various network operations like data storage and retrieval.

## Table of Contents

- [Overview](#overview)
- [Installation](#installation)
- [Usage](#usage)
  - [API Calls](#api-calls)
- [Contributing](#contributing)
  - [Conventional Commits](#conventional-commits)
- [License](#license)

## Installation

To include `sn_client` in your Rust project, add the following to your `Cargo.toml`:

```toml
[dependencies]
sn_client = "latest_version_here"
```

## Usage

To use `sn_client`, you first need to instantiate a client. Here's a simple example:

```rust
use sn_client::Client;
let client = Client::new(signer, peers, req_response_timeout, custom_concurrency_limit).await?;
```

### API Calls

#### `new`

- **Description**: Instantiate a new client.
- **Parameters**:
  - `signer: SecretKey`
  - `peers: Option<Vec<Multiaddr>>`
  - `req_response_timeout: Option<Duration>`
  - `custom_concurrency_limit: Option<usize>`
- **Returns**: `Result<Self>`

#### `get_signed_register_from_network`

- **Description**: Get a register from the network.
- **Parameters**: `address: RegisterAddress`
- **Returns**: `Result<SignedRegister>`

#### `get_register`

- **Description**: Retrieve a Register from the network.
- **Parameters**: `address: RegisterAddress`
- **Returns**: `Result<ClientRegister>`

#### `create_register`

- **Description**: Create a new Register on the Network.
- **Parameters**:
  - `meta: XorName`
  - `verify_store: bool`
- **Returns**: `Result<ClientRegister>`

#### `store_chunk`

- **Description**: Store `Chunk` as a record.
- **Parameters**:
  - `chunk: Chunk`
  - `payment: Vec<CashNote>`
  - `verify_store: bool`
- **Returns**: `Result<()>`

#### `get_chunk`

- **Description**: Retrieve a `Chunk` from the kad network.
- **Parameters**: `address: ChunkAddress`
- **Returns**: `Result<Chunk>`

#### `network_store_spend`

- **Description**: Send a `SpendCashNote` request to the network.
- **Parameters**:
  - `spend: SpendRequest`
  - `verify_store: bool`
- **Returns**: `Result<()>`

#### `get_spend_from_network`

- **Description**: Get a cash_note spend from the network.
- **Parameters**: `cash_note_id: &CashNoteId`
- **Returns**: `Result<SignedSpend>`

#### `get_store_cost_at_address`

- **Description**: Get the store cost at a given address.
- **Parameters**: `address: &NetworkAddress`
- **Returns**: `Result<(PublicAddress, Token)>`

## Contributing

Please refer to the [Contributing Guidelines](../CONTRIBUTING.md) from the main directory for details on how to contribute to this project.

### Conventional Commits

We follow the [Conventional Commits](https://www.conventionalcommits.org/) specification for commit messages. Please adhere to this standard when contributing.

## License

This Safe Network repository is licensed under the General Public License (GPL), version 3 ([LICENSE](LICENSE) http://www.gnu.org/licenses/gpl-3.0.en.html).
