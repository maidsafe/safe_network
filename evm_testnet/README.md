## EVM Testnet

Tool to run a local Ethereum node that automatically deploys all Autonomi smart contracts.

### Requirements

1. Install Foundry to get access to Anvil nodes: https://book.getfoundry.sh/getting-started/installation

### Usage

```bash
cargo run --bin evm_testnet -- --royalties-wallet <ETHEREUM_ADDRESS>
```

Example output:

```
*************************
* Ethereum node started *
*************************
RPC URL: http://localhost:58425/
Payment token address: 0x5FbDB2315678afecb367f032d93F642f64180aa3
Chunk payments address: 0x8464135c8F25Da09e49BC8782676a84730C318bC
```
