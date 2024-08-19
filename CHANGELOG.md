# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

*When editing this file, please respect a line length of 100.*

## 2024-08-27

### Binaries

* `faucet` v0.4.32
* `nat-detection` v0.2.2
* `node-launchpad` v0.3.12
* `safe` v0.94.1
* `safenode` v0.110.1
* `safenode-manager` v0.10.2
* `safenodemand` v0.10.2
* `safenode_rpc_client` v0.6.27
* `sn_auditor` v0.2.4

## 2024-07-25

### Binaries

* `faucet` v0.4.31
* `nat-detection` v0.2.1
* `node-launchpad` v0.3.11
* `safe` v0.94.0
* `safenode` v0.110.0
* `safenode-manager` v0.10.1
* `safenodemand` v0.10.1
* `safenode_rpc_client` v0.6.26
* `sn_auditor` v0.2.3

### 🔦 Highlights

* The introduction of a record-store cache has significantly reduced the node's disk IO. As a side
  effect, the CPU does less work, and performance improves. RAM usage has increased by around 25MB per
  node, but we view this as a reasonable trade off.
* The node's relay server now supports more connections: when running with `--home-network`, up to
  256 will be supported, and otherwise, it will be 1024. Along with minor tweaks to utilize the
  relay server properly, this should hopefully result in less connections being dropped.
* Reward forwarding is more robust.
* Chunk verification is now probabilistic, which should reduce messaging. In combination with
  replication messages also being reduced, this should result in a bandwidth usage reduction of
  ~20%.
* Replication messages are less frequent, reducing bandwidth by ~20% per node. 
* Bad nodes and nodes with a mismatched protocol are now added to a block list. This reduces the
  chance of a network interference and the impact of a bad node in the network.
* For the time being, hole punching has been removed. It was causing handshake time outs, resulting
  in home nodes being less stable. It will be re-enabled in the future.
* Wallet password encryption enhances security, and in the case of secret key leakage, prevents
  unauthorized access.
* Native Apple Silicon (M-series) binaries have been added to our releases, meaning M-series Mac
  users do not have to rely on running Intel binaries with Rosetta.

### Merged Pull Requests

2024-07-11 [#1945](https://github.com/maidsafe/safe_network/pull/1945) -- feat: double spend spam protection

2024-07-11 [#1952](https://github.com/maidsafe/safe_network/pull/1952) -- fix(auditor): create auditor directory if it doesn't exist

2024-07-11 [#1951](https://github.com/maidsafe/safe_network/pull/1951) -- test(spend_simulation): add more attacks

2024-07-11 [#1953](https://github.com/maidsafe/safe_network/pull/1953) -- chore/fix(resources): use more portable shebang

2024-07-12 [#1959](https://github.com/maidsafe/safe_network/pull/1959) -- refactor outdated conn removal

2024-07-12 [#1964](https://github.com/maidsafe/safe_network/pull/1964) -- refactor(cli)!: `wallet address` and `wallet create` changes

2024-07-15 [#1946](https://github.com/maidsafe/safe_network/pull/1946) -- docs(sn_client): Basic documentation

2024-07-15 [#1966](https://github.com/maidsafe/safe_network/pull/1966) -- fix(network): do not add bootstrap peer as relay candidate

2024-07-16 [#1969](https://github.com/maidsafe/safe_network/pull/1969) -- chore(network): force close connection if there is a protocol mistmatch

2024-07-16 [#1972](https://github.com/maidsafe/safe_network/pull/1972) -- feat(safenode_rpc_client): added `--version` flag

2024-07-17 [#1973](https://github.com/maidsafe/safe_network/pull/1973) -- Auditor supplement features

2024-07-17 [#1975](https://github.com/maidsafe/safe_network/pull/1975) -- feat(networking): remove self.close_group and checks there as unused

2024-07-18 [#1976](https://github.com/maidsafe/safe_network/pull/1976) -- chore(networking): make ChunkVerification probabalistic

2024-07-18 [#1949](https://github.com/maidsafe/safe_network/pull/1949) -- feat(wallet): wallet secret key file encryption

2024-07-18 [#1977](https://github.com/maidsafe/safe_network/pull/1977) -- Reduce replication msg processing

2024-07-18 [#1983](https://github.com/maidsafe/safe_network/pull/1983) -- fix(node): remove cn from disk and flush to confirmed_spends during forwarding

2024-07-18 [#1980](https://github.com/maidsafe/safe_network/pull/1980) -- feat(networking): add small record cache

2024-07-18 [#1982](https://github.com/maidsafe/safe_network/pull/1982) -- feat(network): implement blocklist behaviour

2024-07-18 [#1984](https://github.com/maidsafe/safe_network/pull/1984) -- chore(node): move sn_client to dev deps

2024-07-18 [#1985](https://github.com/maidsafe/safe_network/pull/1985) -- Fix Nano count disappearing from Launchpad after restart

2024-07-19 [#1971](https://github.com/maidsafe/safe_network/pull/1971) -- feat!: limit error surface

2024-07-19 [#1986](https://github.com/maidsafe/safe_network/pull/1986) -- Add native Apple Silicon binaries to the release artifacts

2024-07-19 [#1955](https://github.com/maidsafe/safe_network/pull/1955) -- feat(networking): relax relay limits

2024-07-24 [#1990](https://github.com/maidsafe/safe_network/pull/1990) -- chore: implement new process in release workflow

### Detailed Changes

#### Network

##### Added

- Protection against an attack allowing bad nodes or clients to shadow a spend (make it disappear)
  through spamming.
- Nodes allow more relayed connections through them. Also, home nodes will relay through 4 nodes
  instead of 2. Without these changes, relays were denying new connections to home nodes, making them
  difficult to reach.
- Auditor tracks forwarded payments using the default key. 
- Auditor tracks burnt spend attempts and only credits them once.
- Auditor collects balance of UTXOs.
- Added different attack types to the spend simulation test to ensure spend validation is solid.
- Bad nodes and nodes with a mismatched protocol are now added to a block list. This reduces the
  chance of a network interference and the impact of a bad node in the network.
- The introduction of a record-store cache has significantly reduced the node's disk IO. As a side
  effect, the CPU does less work, and performance improves. RAM usage has increased by around 25MB per
  node, but we view this as a reasonable trade off.

##### Changed

- For the time being, hole punching has been removed. It was causing handshake time outs, resulting
  in home nodes being less stable. It will be re-enabled in the future.
- Force connection closure if a peer is using a different protocol.
- Reserve trace level logs for tracking event statistics. Now you can use `SN_LOG=v` to get more
  relevant logs without being overwhelmed by event handling stats.
- Chunk verification is now probabilistic, which should reduce messaging. In combination with
  replication messages also being reduced, this should result in a bandwidth usage reduction of
  ~20%.

##### Fixed

- During payment forwarding, CashNotes are removed from disk and confirmed spends are stored to
  disk. This is necessary for resolving burnt spend attempts for forwarded payments.
- Fix a bug where the auditor was not storing data to disk because of a missing directory.
- Bootstrap peers are not added as relay candidates as we do not want to overwhelm them.

#### Client

##### Added

- Basic global documentation for the `sn_client` crate.
- Option to encrypt the wallet private key with a password, in a file called
  `main_secret_key.encrypted`, inside the wallet directory.
- Option to load a wallet from an encrypted secret-key file using a password.
- The `wallet create` command provides a `--password` argument to encrypt the wallet.
- The `wallet create` command provides a `--no-password` argument skip encryption.
- The `wallet create` command provides a `--no-replace` argument to suppress a prompt to replace an
  existing wallet.
- The `wallet create` command provides a `--key` argument to create a wallet from a hex-encoded
  private key.
- The `wallet create` command provides a `--derivation` argument to set a derivation passphrase to
  be used with the mnemonic to create a new private key.
- A new `wallet encrypt` command encrypts an existing wallet.

##### Changed

- The `wallet address` command no longer creates a new wallet if no wallet exists.
- The `wallet create` command creates a wallet using the account mnemonic instead of requiring a
  hex-encoded secret key.
- The `wallet create` `--key` and `--derivation` arguments are mutually exclusive.

#### Launchpad

##### Fixed

- The `Total Nanos Earned` stat no longer resets on restart.

#### RPC Client

##### Added

- A `--version` argument shows the binary version

#### Other

##### Added

- Native Apple Silicon (M-series) binaries have been added to our releases, meaning M-series Mac
  users do not have to rely on running Intel binaries with Rosetta.

## 2024-07-10

### Binaries

* `faucet` v0.4.30
* `nat-detection` v0.2.0
* `node-launchpad` v0.3.10
* `safe` v0.93.9
* `safenode` v0.109.0
* `safenode-manager` v0.10.0
* `sn_auditor` v0.2.2
* `sn_node_rpc_client` v0.6.25

### Network

#### Added

- The node exposes more metrics, including its uptime, number of connected peers, number of peers in
  the routing table, and the number of open connections. These will help us more effectively
  diagnose user issues.

#### Changed

- Communication between node and client is strictly limited through synchronised public keys. The
  current beta network allows the node and client to use different public keys, resulting in
  undefined behaviour and performance issues. This change mitigates some of those issues and we also
  expect it to prevent other double spend issues.
- Reduced base traffic for nodes, resulting in better upload performance. This will result in better
  distribution of nanos, meaning users with a smaller number of nodes will be expected to receive
  nanos more often.

#### Fixed

- In the case where a client retries a failed upload, they would re-send their payment. In a rare
  circumstance, the node would forward this reward for a second time too. This is fixed on the node.
- Nodes are prevented from double spending under rare circumstances.
- ARM builds are no longer prevented from connecting to the network.

### Node Manager

#### Added

- Global `--debug` and `--trace` arguments are provided. These will output debugging and trace-level
  logging, respectively, direct to stderr.

#### Changed

- The mechanism used by the node manager to refresh its state is significantly changed to address
  issues that caused commands to hang for long periods of time. Now, when using commands like
  `start`, `stop`, and `reset`, users should no longer experience the commands taking excessively
  long to complete.
- The `nat-detection run` command provides a default list of servers, meaning the `--servers`
  argument is now optional.

### Launchpad

#### Added

- Launchpad and node versions are displayed on the user interface.

#### Changed

- The node manager change for refreshing its state also applies to the launchpad. Users should
  experience improvements in operations that appeared to be hanging but were actually just taking
  an excessive amount of time to complete.

#### Fixed

- The correct primary storage will now be selected on Linux and macOS.
