# Safe Network Node (sn_node)

## Overview

The `sn_node` directory provides the `safenode` binary and Python bindings for the Safe Network node implementation. This directory contains the core logic for node operations, including API definitions, error handling, event management, and data validation.

## Table of Contents

- [Overview](#overview)
- [Installation](#installation)
- [Usage](#usage)
  - [Binary Usage](#binary-usage)
  - [Python Usage](#python-usage)
- [Directory Structure](#directory-structure)
- [Testing](#testing)
- [Contributing](#contributing)
- [License](#license)

## Installation

### Binary Installation
Follow the main project's installation guide to set up the `safenode` binary.

### Python Installation
To install the Python bindings, you'll need:
- Python 3.8 or newer
- Rust toolchain
- maturin (`pip install maturin`)

Install the package using:
```bash
maturin develop
```

## Usage

### Binary Usage
To run the `safenode` binary, follow the instructions in the main project's usage guide.

### Python Usage
The Python module provides a simple interface to run and manage Safe Network nodes. Here's a basic example:

```python
from safenode import SafeNode

# Example initial peers (note: these are example addresses and may not be active)
# You should use current active peers from the network
initial_peers = [
    "/ip4/142.93.37.4/udp/40184/quic-v1/p2p/12D3KooWPC8q7QGZsmuTtCYxZ2s3FPXPZcS8LVKkayXkVFkqDEQB",
    "/ip4/157.245.40.2/udp/33698/quic-v1/p2p/12D3KooWNyNNTGfwGf6fYyvrk4zp5EHxPhNDVNB25ZzEt2NXbCq2",
    "/ip4/157.245.40.2/udp/33991/quic-v1/p2p/12D3KooWHPyZVAHqp2ebzKyxxsYzJYS7sNysfcLg2s1JLtbo6vhC"
]

# Create and start a node
node = SafeNode()
node.run(
    rewards_address="0x1234567890123456789012345678901234567890",  # Your EVM wallet address
    evm_network="arbitrum_sepolia",  # or "arbitrum_one" for mainnet
    ip="0.0.0.0",
    port=12000,
    initial_peers=initial_peers,
    local=False,
    root_dir=None,  # Uses default directory
    home_network=False
)

# Get node information
peer_id = node.peer_id()
print(f"Node peer ID: {peer_id}")

# Get current rewards address
address = node.get_rewards_address()
print(f"Current rewards address: {address}")

# Get network information
kbuckets = node.get_kbuckets()
for distance, peers in kbuckets:
    print(f"Distance {distance}: {len(peers)} peers")
```

#### Available Methods
- `run()`: Start the node with configuration
- `peer_id()`: Get the node's peer ID
- `get_rewards_address()`: Get the current rewards/wallet address
- `set_rewards_address()`: Set a new rewards address (requires node restart)
- `get_all_record_addresses()`: Get all record addresses stored by the node
- `get_kbuckets()`: Get routing table information

#### Important Notes
- The initial peers list needs to contain currently active peers from the network
- The rewards address should be a valid EVM address
- Changing the rewards address requires restarting the node
- The node needs to connect to active peers to participate in the network

## Directory Structure

- `src/`: Source code files
  - `api.rs`: API definitions
  - `error.rs`: Error types and handling
  - `event.rs`: Event-related logic
  - `get_validation.rs`: Validation for GET requests
  - `put_validation.rs`: Validation for PUT requests
  - `replication.rs`: Data replication logic
  - `spends.rs`: Logic related to spending tokens or resources
- `tests/`: Test files
  - `common/mod.rs`: Common utilities for tests
  - `data_with_churn.rs`: Tests related to data with churn
  - `sequential_transfers.rs`: Tests for sequential data transfers
  - `storage_payments.rs`: Tests related to storage payments
  - `verify_data_location.rs`: Tests for verifying data locations

## Testing

To run tests, navigate to the `sn_node` directory and execute:

```bash
cargo test
```

## Contributing

Please feel free to clone and modify this project. Pull requests are welcome.

## Conventional Commits

We follow the [Conventional Commits](https://www.conventionalcommits.org/) specification for all commits. Make sure your commit messages adhere to this standard.

## License

This Safe Network repository is licensed under the General Public License (GPL), version 3 ([LICENSE](LICENSE) http://www.gnu.org/licenses/gpl-3.0.en.html).


