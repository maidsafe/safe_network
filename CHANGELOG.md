# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

*When editing this file, please respect a line length of 100.*

## 2024-12-21

### Network

#### Fixed

- Do not dial back when a new peer is detected. This resulted in a large number of open connections,
  in turn causing increased CPU usage.

### Client

#### Changed

- Remove the 'dial error' output on the `file upload` command

## 2024-12-18

### General

#### Changed

- For a branding alignment that moves Safe Network to Autonomi, all crates in the workspace prefixed
  `sn-` were renamed with an `ant-` prefix. For example, `sn-node` was renamed `ant-node`.
- To further support this alignment, several binaries were renamed:
   + `autonomi` -> `ant`
   + `safenode` -> `antnode`
   + `safenode-manager` -> `antctl`
   + `safenode_rpc_client` -> `antnode_rpc_client`
- The location of data directories used by the binaries were changed from `~/.local/share/safe` to
  `~/.local/share/autonomi`. The same is true of the equivalent locations on macOS and Windows.
- The prefixes of metric names in the `safenode` binary (now `antnode`) were changed from `sn_` to
  `ant_`.

### Network

#### Added

- Provide Python bindings for `antnode`.
- Generic `Transaction` data type
- Upgraded quoting with smart-contract-based pricing. This makes pricing fairer, as more nodes
  are rewarded and there are less incentives to cheat.
- Upgraded data payments verification.
- New storage proof verification which attempts to avoid outsourcing attack
- RBS support, dynamic `responsible_range` based on `network_density` equation estimation.
- Node support for client’s RBS `get_closest` query.
- More quoting metrics for potential future quoting scheme.
- Implement bootstrap cache for local, decentralized network contacts.
- Increased the number of peers returned for the `get_closest` query result.

#### Changed

- The `SignedSpend` data type was replaced by `Transaction`.
- Removed `group_consensus` on `BadNode` to support RBS in the future.
- Removed node-side quoting history check as part of the new quoting scheme.
- Rename `continuous_bootstrap` to `network_discovery`.
- Convert `Distance` into `U256` via output string. This avoids the need to access the
  `libp2p::Distance` private field because the change for it has not been published yet.
- For node and protocol versioning we remove the use of various keys in favour of a simple 
  integer between `0` and `255`. We reserve the value `1` for the main production network.
- The `websockets` feature was removed from the node binary. We will no longer support the `ws`
  protocol for connections.

#### Fixed

- Populate `records_by_bucket` during restart so that proper quoting can be retained after restart.
- Scramble `libp2p` native bootstrap to avoid patterned spike of resource usage.
- Replicate fresh `ScratchPad`
- Accumulate and merge `ScratchPad` on record get. 
- Remove an external address if it is unreliable.
- Bootstrap nodes were being replaced too frequently in the routing table.

### Client

#### Added

- Provide Python bindings.
- Support for generic `Transaction` data type.
- Upgraded quoting with smart contract.
- Upgraded data payments with new quoting.
- Retry failed PUTs. This will retry when chunks failed to upload.
- WASM function to generate a vault key from a wallet signature.
- Use bootstrap cache mechanism to initialize `Client` object. 
- Exposed many types at top-level, for more ergonomic use of the API. Together with more examples on
  function usage.
- Deprecated registers for the client, planning on replacing them fully with transactions and
  pointers.
- Wait a short while for initial network discovery to settle before quoting or uploading tasks
  begin.
- Stress tests for the register features of the vault.
- Improved logging for vault end-to-end test cases.
- More debugging logging for the client API and `evmlib`.
- Added support for adding a wallet from an environment variable if no wallet files are present.
- Provide `wallet export` command to export a wallet’s private key

#### Changed

- Added and modified documentation in various places to improve developer experience.
- Renamed various methods to 'default' to private uploading, while public will have `_public`
  suffixed. Also has various changes to allow more granular uploading of archives and data maps.
- Archives now store relative paths to files instead of absolute paths.
- The `wallet create --private-key` command has been changed to `wallet import`.

#### Fixed

- Files now download to a specific destination path.
- Retry when the number of quotes obtained are not enough.
- Return the wallet from an environment variable rather than creating a file.
- Error when decrypting a wallet that was imported without the `0x` prefix.
- Issue when selecting a wallet that had multiple wallet files (unencrypted & encrypted).

### Launchpad

#### Added

- Added `--network-id` and `--antnode-path` args for testing

## 2024-11-25

### Network

#### Fixed

- Make native kad bootstrap interval more random. So that when running multiple nodes
  on one machine, there is no resource usage spike appears with fixed interval.

## 2024-11-13

### Network

#### Fixed

- During a restart, the node builds a cache of locally restored records,
  which is used to improve the speed of the relevant records calculation.
  The restored records were not being added to the cache.
  This has now been corrected.

## 2024-11-12

### Network

#### Added

- Enable the `websockets` connection feature, for compatibility with the webapp.

#### Fixed

- Reduce incorrect logging of connection errors.
- Fixed verification for crdt operations.
- Pick chunk-proof verification (for storage confirmation) candidates more equally.

### Launchpad

#### Added

- Display an error when Launchpad is not whitelisted on Windows devices.
- Ctrl+V can paste rewards address on pop up section.

#### Changed

- Help section copy changed after beta phase.
- Update ratatui and throbbber library versions.

#### Fixed

- We display starting status when not running nodes

### Client

#### Added

- Support pre-paid put operations.
- Add the necessary WASM bindings for the webapp to be able to upload private data to a vault
  and fetch it again.

#### Changed

- Chunks are now downloaded in parallel.
- Rename some WASM methods to be more conventional for web.

## 2024-11-07

### Launchpad

#### Added

- You can select a node. Pressing L will show its logs.
- The upgrade screen has an estimated time.

#### Changed

- Launchpad now uses multiple threads. This allows the UI to be functional while nodes are being
  started, upgraded, and so on.
- Mbps vs Mb units on status screen.

#### Fixed

- Spinners now move when updating.

## 2024-11-06

### Network

#### Added

- Remove outdated record copies that cannot be decrypted. This is used when a node is restarted.

#### Changed

- The node will only restart at the end of its process if it has explicitly been requested in the
  RPC restart command. This removes the potential for creation of undesired new processes.
- Range search optimization to reduce resource usage.
- Trigger record_store pruning earlier. The threshold lowered from 90% to 10% to improve the disk
  usage efficiency.

#### Fixed

- Derive node-side record encryption details from the node's keypair. This ensures data is retained
  in a restart.

### Client

#### Changed

- When paying for quotes through the API, the contract allowance will be set to ~infinite instead of
  the specific amount needed. This is to reduce the amount of approval transactions needed for doing
  quote payments.

### Node Manager

#### Fixed

- The `--rewards-address` argument is retained on an upgrade

### Launchpad

#### Added

- Support for upgrading nodes version
- Support for Ctrl+V on rewards address
- More error handling
- Use 5 minute interval between upgrades

#### Changed

- Help screen after beta
- New Ratatui version 0.29.0

## 2024-10-28

### Autonomi API/CLI

#### Added 

- Private data support.
- Local user data support.
- Network Vault containing user data encrypted.
- Archives with Metadata.
- Prepaid upload support for data_put using receipts.

#### Changed

- Contract token approval amount set to infinite before doing data payments.

### Client

#### Added

- Expose APIs in WASM (e.g. archives, vault and user data within vault).
- Uploads are not run in parallel.
- Support for local wallets.
- Provide `wallet create` command.
- Provide `wallet balance` command.

#### Changed

- Take metadata from file system and add `uploaded` field for time of upload.

#### Fixed

- Make sure we use the new client path throughout the codebase

### Network

#### Added

- Get range used for store cost and register queries.
- Re-enabled large_file_upload, memcheck, benchmark CI tests.

#### Changed

- Scratchpad modifications to support multiple data encodings.
- Registers are now merged at the network level, preventing failures during update and during
  replication.
- Libp2p config and get range tweaks reduce intensity of operations. Brings down CPU usage
  considerably.
- Libp2p’s native kad bootstrap interval introduced in 0.54.1 is intensive, and as we roll our own,
  we significantly reduce the kad period to lighten the CPU load.
- Wipe node’s storage dir when restarting for new network

#### Fixed

- Fixes in networking code for WASM compatibility (replacing `std::time` with compatible
  alternative).
- Event dropped errors should not happen if the event is not dropped.
- Reduce outdated connection pruning frequency.

### Node Manager

#### Fixed

- Local node register is cleaned up when --clean flag applied (prevents some errors when register
  changes).

### Launchpad

#### Fixed

- Status screen is updated after nodes have been reset.
- Rewards Address is required before starting nodes. User input is required.
- Spinner does not stop spinning after two minutes when nodes are running.

## 2024-10-24

### Network

#### Changed

- The `websockets` feature is removed because it was observed to cause instability.

### Client

#### Changed

- PR #2281 was reverted to restore prior behaviour.

### Launchpad

#### Changed

- The Discord username was replaced with the rewards address.
- Remove the reject terms and conditions pop-up screen.

## 2024-10-22

Unfortunately the entry for this release will not have fully detailed changes. This release is
special in that it's very large and moves us to a new, EVM-based payments system. The Github Release
description has a list of all the merged PRs. If you want more detail, consult the PR list. Normal
service will resume for subsequent releases.

Here is a brief summary of the changes:

- A new `autonomi` CLI that uses EVM payments and replaces the previous `safe` CLI.
- A new `autonomi` API that replaces `sn_client` with a simpler interface.
- The node has been changed to use EVM payments.
- The node runs without a wallet. This increases security and removes the need for forwarding.
- Data is paid for through an EVM smart contract. Payment proofs are not linked to the original
  data.
- Payment royalties have been removed, resulting in less centralization and fees.

## 2024-10-08

### Network

#### Changed

- Optimize auditor tracking by not to re-attempt fetched spend.
- Optimize auditor tracking function by using DashMap and stream.

## 2024-10-07

### Network

#### Changed

- Increase chunk size to 4MB with node size remaining at 32GB
- Bootstrap peer parsing in CI was changed to accommodate new log format in libp2p

### Node Manager

#### Added

- The `add` command has new `--max-log-files` and `--max-archived-log-files` arguments to support
  capping node log output

#### Fixed

- The Discord username on the `--owner` argument will always be converted to lower case

#### Launchpad

### Added

- Increased logging related to app configuration. This could help solving issues on launchpad start
  up.

## 2024-10-03

### Launchpad

### Changed

- Upgrade to `Ratatui` v0.28.1
- Styling and layout fixes

#### Added

- Drives that don't have enough space are being shown and flagged
- Error handling and generic error popup
- New metrics in the `Status` section
- Confirmation needed when changing connection mode

### Fixed

- NAT mode only on first start in `Automatic Connection Mode`
- Force Discord username to be in lowercase

## 2024-10-01

### Launchpad

#### Changed

- Disable node selection on status screen
- We change node size from 5GB to 35GB

## 2024-10-01

### Network

#### Changed

- Increase node storage size from 2GB to 32GB

## 2024-09-24

### Network

#### Fixed

- The auditor now uses width-first tracking, to bring it in alignment with the new wallet.

### Client

#### Added

- The client will perform quote validation to avoid invalid quotes.
- A new high-level client API, `autonomi`. The crate provides most of the features necessary to
  build apps for the Autonomi network.

### Node Manager

#### Fixed

- The node manager status command was not functioning correctly when used with a local network. The
  mechanism for determining whether a node was running was changed to use the path of the service
  process, but this did not work for a local network. The status command now differentiates between
  a local and a service-based network, and the command now behaves as expected when using a local
  network.

### Documentation

- In the main README for the repository, the four network keys were updated to reflect the keys
  being used  by the new stable network.

## 2024-09-12

### Network

#### Changed

- The circuit-bytes limit is increased. This enables `libp2p-relay` to forward large records, such
  as `ChunkWithPayment`, enabling home nodes to be notified that they have been paid.

## 2024-09-09

### Network

#### Added

- More logging for storage errors and setting the responsible range.

#### Changed

- The node's store cost calculation has had various updates:
    + The minimum and maximum were previously set to 10 and infinity. They've now been updated to 1
      and 1 million, respectively.
    + We are now using a sigmoid curve, rather than a linear curve, as the base curve. The previous
      curve only grew steep when the storage capacity was 40 to 60 percent.
    + The overall calculation is simplified.
- We expect the updates to the store cost calculation to prevent 'lottery' payments, where one node
  would have abnormally high earnings.
- The network version string, which is used when both nodes and clients connect to the network, now
  uses the version number from the `sn_protocol` crate rather than `sn_networking`. This is a
  breaking change in `sn_networking`.
- External address management is improved. Before, if anyone observed us at a certain public
  IP+port, we would trust that and add it if it matches our local port. Now, we’re keeping track and
  making sure we only have a single external address that we set when we’ve been observed as that
  address a certain amount of times (3 by default). It should even handle cases where our IP changes
  because of (mobile) roaming.
- The `Spend` network data type has been refactored to make it lighter and simpler.
- The entire transaction system has been redesigned; the code size and complexity have been reduced
  by an order of magnitude.
- In addition, almost 10 types were removed from the transaction code, further reducing the
  complexity.
- The internals of the `Transfer` and `CashNote` types have been reworked.
- The replication range has been reduced, which in turn reduces the base traffic for replication.

### Client

#### Fixed

- Registers are fetched and merged correctly. 

### Launchpad

#### Added

- A connection mode feature enables users to select whether they want their nodes to connect to the
  network using automatic NAT detection, upnp, home network, or custom port mappings in their
  connection. Previously, the launchpad used NAT detection on the user’s behalf. By providing the
  ability to explore more connection modes, hopefully this will get more users connected.

#### Changed

- On the drive selection dialog, drives to which the user does not have read or write access are
  marked as such.

### Documentation

#### Added

- A README was provided for the `sn_registers` crate. It intends to give a comprehensive
  understanding of the register data type and how it can be used by developers.

#### Changed

- Provided more information on connecting to the network using the four keys related to funds, fees
  and royalties.

## 2024-09-02

### Launchpad

#### Fixed

- Some users encountered an error when the launchpad started, related to the storage mountpoint not
  being set. We fix the error by providing default values for the mountpoint settings when the
  `app_data.json` file doesn't exist (fresh install). In the case where it does exist, we validate
  the contents.

## 2024-08-27

### Network

#### Added

- The node will now report its bandwidth usage through the metrics endpoint.
- The metrics server has a new `/metadata` path which will provide static information about the node,
  including peer ID and version.
- The metrics server exposes more metrics on store cost derivation. These include relevant record
  count and number of payments received.
- The metrics server exposes metrics related to bad node detection.
- Test to confirm main key can’t verify signature signed by child key.
- Avoid excessively high quotes by pruning records that are not relevant.

#### Changed

- Bad node detection and bootstrap intervals have been increased. This should reduce the number
  of messages being sent.
- The spend parent verification strategy was refactored to be more aligned with the public
  network.
- Nodes now prioritize local work over new work from the network, which reduces memory footprint.
- Multiple GET queries to the same address are now de-duplicated and will result in a single query
  being processed.
- Improve efficiency of command handling and the record store cache.
- A parent spend is now trusted with a majority of close group nodes, rather than all of them. This
  increases the chance of the spend being stored successfully when some percentage of nodes are slow
  to respond.

#### Fixed

- The amount of bytes a home node could send and receive per relay connection is increased. This
  solves a problem where transmission of data is interrupted, causing home nodes to malfunction.
- Fetching the network contacts now times out and retries. Previously we would wait for an excessive
  amount of time, which could cause the node to hang during start up.
- If a node has been shunned, we inform that node before blocking all communication to it.
- The current wallet balance metric is updated more frequently and will now reflect the correct
  state.
- Avoid burnt spend during forwarding by correctly handling repeated CashNotes and confirmed spends.
- Fix logging for CashNote and confirmed spend disk ops
- Check whether a CashNote has already been received to avoid duplicate CashNotes in the wallet.

### Node Manager

#### Added

- The `local run` command supports `--metrics-port`, `--node-port` and `--rpc-port` arguments.
- The `start` command waits for the node to connect to the network before attempting to start the
  next node. If it takes more than 300 seconds to connect, we consider that a failure and move to the
  next node. The `--connection-timeout` argument can be used to vary the timeout. If you prefer the
  old behaviour, you can use the `--interval` argument, which will continue to apply a static,
  time-based interval.

#### Changed

- On an upgrade, the node registry is saved after each node is processed, as opposed to waiting
  until the end. This means if there is an unexpected failure, the registry will have the
  information about which nodes have already been upgraded.

### Launchpad

#### Added

- The user can choose a different drive for the node's data directory.
- New sections in the UI: `Options` and `Help`.
- A navigation bar has been added with `Status`, `Options` and `Help` sections.
- The node's logs can be viewed from the `Options` section.

#### Changed

- Increased spacing for title and paragraphs.
- Increased spacing on footer.
- Increased spacing on box titles.
- Moved `Discord Username` from the top title into the `Device Status` section.
- Made the general layout of `Device Status` more compact.

### Client

#### Added

- The `safe files download` command now displays duration per file.

#### Changed

- Adjust the put and get configuration scheme to align the client with a more realistic network
  which would have some percentage of slow nodes.
- Improved spend logging to help debug the upload process.

#### Fixed

- Avoid a corrupt wallet by terminating the payment process during an unrecoverable error.

## 2024-07-25

### Network

#### Added

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

#### Changed

- For the time being, hole punching has been removed. It was causing handshake time outs, resulting
  in home nodes being less stable. It will be re-enabled in the future.
- Force connection closure if a peer is using a different protocol.
- Reserve trace level logs for tracking event statistics. Now you can use `SN_LOG=v` to get more
  relevant logs without being overwhelmed by event handling stats.
- Chunk verification is now probabilistic, which should reduce messaging. In combination with
  replication messages also being reduced, this should result in a bandwidth usage reduction of
  ~20%.

#### Fixed

- During payment forwarding, CashNotes are removed from disk and confirmed spends are stored to
  disk. This is necessary for resolving burnt spend attempts for forwarded payments.
- Fix a bug where the auditor was not storing data to disk because of a missing directory.
- Bootstrap peers are not added as relay candidates as we do not want to overwhelm them.

### Client

#### Added

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

#### Changed

- The `wallet address` command no longer creates a new wallet if no wallet exists.
- The `wallet create` command creates a wallet using the account mnemonic instead of requiring a
  hex-encoded secret key.
- The `wallet create` `--key` and `--derivation` arguments are mutually exclusive.

### Launchpad

#### Fixed

- The `Total Nanos Earned` stat no longer resets on restart.

### RPC Client

#### Added

- A `--version` argument shows the binary version

### Other

#### Added

- Native Apple Silicon (M-series) binaries have been added to our releases, meaning M-series Mac
  users do not have to rely on running Intel binaries with Rosetta.

## 2024-07-10

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
