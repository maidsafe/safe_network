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

## Contributing

Please refer to the [Contributing Guidelines](../CONTRIBUTING.md) from the main directory for details on how to contribute to this project.

### Conventional Commits

We follow the [Conventional Commits](https://www.conventionalcommits.org/) specification for commit messages. Please adhere to this standard when contributing.

## License

This Safe Network repository is licensed under the General Public License (GPL), version 3 ([LICENSE](LICENSE) http://www.gnu.org/licenses/gpl-3.0.en.html).
