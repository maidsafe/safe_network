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

The Python module provides a comprehensive interface to run and manage Safe Network nodes. Here's a complete overview:

#### Basic Node Operations

```python
from safenode import SafeNode

# Create and start a node
node = SafeNode()
node.run(
    rewards_address="0x1234567890123456789012345678901234567890",  # Your EVM wallet address
    evm_network="arbitrum_sepolia",  # or "arbitrum_one" for mainnet
    ip="0.0.0.0",
    port=12000,
    initial_peers=[
        "/ip4/142.93.37.4/udp/40184/quic-v1/p2p/12D3KooWPC8q7QGZsmuTtCYxZ2s3FPXPZcS8LVKkayXkVFkqDEQB",
    ],
    local=False,
    root_dir=None,  # Uses default directory
    home_network=False
)
```

#### Available Methods

Node Information:

- `peer_id()`: Get the node's peer ID
- `get_rewards_address()`: Get current rewards/wallet address
- `set_rewards_address(address: str)`: Set new rewards address (requires restart)
- `get_kbuckets()`: Get routing table information
- `get_all_record_addresses()`: Get all stored record addresses

Storage Operations:

- `store_record(key: str, value: bytes, record_type: str)`: Store data
  - `key`: Hex string
  - `value`: Bytes to store
  - `record_type`: "chunk" or "scratchpad"
- `get_record(key: str) -> Optional[bytes]`: Retrieve stored data
- `delete_record(key: str) -> bool`: Delete stored data
- `get_stored_records_size() -> int`: Get total size of stored data

Directory Management:

- `get_root_dir() -> str`: Get current root directory path
- `get_default_root_dir(peer_id: Optional[str]) -> str`: Get default root directory
- `get_logs_dir() -> str`: Get logs directory path
- `get_data_dir() -> str`: Get data storage directory path

#### Storage Example

```python
# Store some data
key = "1234567890abcdef"  # Hex string key
data = b"Hello, Safe Network!"
node.store_record(key, data, "chunk")

# Retrieve the data
stored_data = node.get_record(key)
if stored_data:
    print(f"Retrieved: {stored_data.decode()}")

# Get storage info
size = node.get_stored_records_size()
print(f"Total storage used: {size} bytes")

# Delete data
if node.delete_record(key):
    print("Data deleted successfully")
```

#### Directory Management Example

```python
# Get various directory paths
root_dir = node.get_root_dir()
logs_dir = node.get_logs_dir()
data_dir = node.get_data_dir()

# Get default directory for a specific peer
default_dir = SafeNode.get_default_root_dir(peer_id)
```

#### Important Notes

- Initial peers list should contain currently active network peers
- Rewards address must be a valid EVM address
- Changing rewards address requires node restart
- Storage keys must be valid hex strings
- Record types are limited to 'chunk' and 'scratchpad'
- Directory paths are platform-specific
- Custom root directories can be set at node startup

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


