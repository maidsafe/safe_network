# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.8.0](https://github.com/joshuef/safe_network/compare/sn-node-manager-v0.7.7...sn-node-manager-v0.8.0) - 2024-05-24

### Added
- *(node_manager)* pass beta encryption sk to the auditor
- *(manager)* maintain n running nodes
- provide `local status` command
- provide `--owner` arg for `add` cmd
- *(nodeman)* add LogFormat as a startup arg for nodes
- *(node_manager)* add auditor support
- provide `--upnp` flag for `add` command
- *(launchpad)* initial automatic resource allocation logic
- *(audit)* collect payment forward statistics
- run safenode services in user mode
- provide `autonomi-launcher` binary
- *(manager)* reuse downloaded binaries
- *(launchpad)* remove nodes
- *(tui)* adding services
- *(node)* make spend and cash_note reason field configurable
- [**breaking**] provide `--home-network` arg for `add` cmd
- provide `--interval` arg for `upgrade` cmd
- provide `--path` arg for `upgrade` cmd
- rpc restart command
- provide `reset` command
- provide `balance` command
- make `--peer` argument optional
- distinguish failure to start during upgrade
- use different key for payment forward
- hide genesis keypair
- *(node)* periodically forward reward to specific address
- spend reason enum and sized cipher
- *(network)* add --upnp flag to node
- spend shows the purposes of outputs created for
- *(relay)* remove autonat and enable hole punching manually
- *(relay)* impl RelayManager to perform circuit relay when behind NAT
- *(node)* notify peer it is now considered as BAD
- *(networking)* shift to use ilog2 bucket distance for close data calcs
- unit testing dag, double spend poisoning tweaks
- report protocol mismatch error
- use default keys for genesis, or override
- remove two uneeded env vars
- pass genesis_cn pub fields separate to hide sk
- hide genesis keypair
- pass sk_str via cli opt
- *(node)* use separate keys of Foundation and Royalty
- *(wallet)* ensure genesis wallet attempts to load from local on init first
- *(faucet)* make gifting server feat dependent
- tracking beta rewards from the DAG

### Fixed
- avoid adding mixed type addresses into RT
- *(manager)* download again if cached archive is corrupted
- check node registry exists before deleting it
- retain options on upgrade and prevent dup ports
- *(manager)* do not print to stdout on low verbosity level
- do not create wallet on registry refresh
- change reward balance to optional
- apply interval only to non-running nodes
- do not delete custom bin on `add` cmd
- incorrect release type reference
- use correct release type in upgrade process
- *(node)* notify fetch completion earlier to avoid being skipped
- create faucet via account load or generation
- more test and cli fixes
- update calls to HotWallet::load
- do not add reported external addressese if we are behind home network
- *(node)* notify replication_fetcher of early completion
- *(node)* not send out replication when failed read from local
- correct genesis_pk naming
- genesis_cn public fields generated from hard coded value
- invalid spend reason in data payments

### Other
- update based on comment
- enable node man integration tests
- *(release)* sn_auditor-v0.1.16/sn_cli-v0.91.4/sn_faucet-v0.4.18/sn_metrics-v0.1.7/sn_node-v0.106.4/sn_service_management-v0.2.8/node-launchpad-v0.1.5/sn-node-manager-v0.7.7/sn_node_rpc_client-v0.6.17
- use owners on memcheck workflow local network
- reconfigure local network owner args
- *(nodemanager)* upgrade_should_retain_the_log_format_flag
- use helper function to print banners
- use published versions of deps
- *(release)* node-launchpad-v0.1.3/sn-node-manager-v0.7.6
- *(release)* sn_auditor-v0.1.15/sn_cli-v0.91.3/sn_faucet-v0.4.17/sn_metrics-v0.1.6/sn_node-v0.106.3/sn_service_management-v0.2.7/node-launchpad-v0.1.2/sn_node_rpc_client-v0.6.16
- use const for default user or owner
- resolve errors after reverts
- Revert "feat(node): make spend and cash_note reason field configurable"
- change terminal launch behaviour
- update cli and readme for user-mode services
- upgrade service manager crate
- *(release)* sn_auditor-v0.1.13/sn_client-v0.106.1/sn_networking-v0.15.1/sn_protocol-v0.16.6/sn_cli-v0.91.1/sn_faucet-v0.4.15/sn_node-v0.106.1/node-launchpad-v0.1.1/sn_node_rpc_client-v0.6.14/sn_peers_acquisition-v0.2.12/sn_service_management-v0.2.6
- *(release)* sn_auditor-v0.1.12/sn_client-v0.106.0/sn_networking-v0.15.0/sn_transfers-v0.18.0/sn_peers_acquisition-v0.2.11/sn_logging-v0.2.26/sn_cli-v0.91.0/sn_faucet-v0.4.14/sn_metrics-v0.1.5/sn_node-v0.106.0/sn_service_management-v0.2.5/test_utils-v0.4.1/node-launchpad-v/sn-node-manager-v0.7.5/sn_node_rpc_client-v0.6.13/token_supplies-v0.1.48/sn_protocol-v0.16.5
- *(versions)* sync versions with latest crates.io vs for nodeman
- *(versions)* sync versions with latest crates.io vs
- use node registry for status
- [**breaking**] output reward balance in `status --json` cmd
- use better banners
- properly use node registry and surface peer ids if they're not
- `remove` cmd operates over all services
- provide `local` subcommand
- *(release)* sn_auditor-v0.1.7/sn_client-v0.105.3/sn_networking-v0.14.4/sn_protocol-v0.16.3/sn_build_info-v0.1.7/sn_transfers-v0.17.2/sn_peers_acquisition-v0.2.10/sn_cli-v0.90.4/sn_faucet-v0.4.9/sn_metrics-v0.1.4/sn_node-v0.105.6/sn_service_management-v0.2.4/sn-node-manager-v0.7.4/sn_node_rpc_client-v0.6.8/token_supplies-v0.1.47
- *(deps)* bump dependencies
- *(node)* log node owner
- make open metrics feature default but without starting it by default
- *(refactor)* stabilise node size to 4k records,
- Revert "feat: spend shows the purposes of outputs created for"
- Revert "chore: rename output reason to purpose for clarity"
- *(node)* use proper SpendReason enum
- *(release)* sn_client-v0.106.2/sn_networking-v0.15.2/sn_cli-v0.91.2/sn_node-v0.106.2/sn_auditor-v0.1.14/sn_faucet-v0.4.16/sn_node_rpc_client-v0.6.15
- *(release)* sn_registers-v0.3.13
- *(node)* make owner optional
- cargo fmt
- rename output reason to purpose for clarity
- store owner info inside node instead of network
- *(CI)* upload faucet log during CI
- *(node)* lower some log levels to reduce log size
- *(CI)* confirm there is no failed replication fetch
- *(node)* pass entire QuotingMetrics into calculate_cost_for_records
- *(transfers)* comment and naming updates for clarity
- log genesis PK
- rename improperly named foundation_key
- Revert "feat(cli): track spend creation reasons during audit"
- Revert "chore: refactor CASH_NOTE_REASON strings to consts"
- Revert "chore: address review comments"
- add consts

## [0.7.7](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.7.6...sn-node-manager-v0.7.7) - 2024-05-20

### Added
- *(node_manager)* add auditor support
- provide `--upnp` flag for `add` command

### Fixed
- retain options on upgrade and prevent dup ports

### Other
- use helper function to print banners
- use published versions of deps
- update Cargo.lock dependencies

## [0.7.6](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.7.5...sn-node-manager-v0.7.6) - 2024-05-15

### Added
- *(launchpad)* initial automatic resource allocation logic
- run safenode services in user mode

### Other
- *(release)* sn_auditor-v0.1.15/sn_cli-v0.91.3/sn_faucet-v0.4.17/sn_metrics-v0.1.6/sn_node-v0.106.3/sn_service_management-v0.2.7/node-launchpad-v0.1.2/sn_node_rpc_client-v0.6.16
- change terminal launch behaviour
- update cli and readme for user-mode services
- upgrade service manager crate
- *(release)* sn_auditor-v0.1.13/sn_client-v0.106.1/sn_networking-v0.15.1/sn_protocol-v0.16.6/sn_cli-v0.91.1/sn_faucet-v0.4.15/sn_node-v0.106.1/node-launchpad-v0.1.1/sn_node_rpc_client-v0.6.14/sn_peers_acquisition-v0.2.12/sn_service_management-v0.2.6

## [0.7.5-alpha.4](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.7.5-alpha.3...sn-node-manager-v0.7.5-alpha.4) - 2024-05-07

### Added
- provide `autonomi-launcher` binary
- *(manager)* reuse downloaded binaries
- *(launchpad)* remove nodes
- *(tui)* adding services
- *(node)* make spend and cash_note reason field configurable
- [**breaking**] provide `--home-network` arg for `add` cmd
- provide `--interval` arg for `upgrade` cmd
- provide `--path` arg for `upgrade` cmd
- rpc restart command
- provide `reset` command
- provide `balance` command
- make `--peer` argument optional
- distinguish failure to start during upgrade

### Fixed
- *(manager)* do not print to stdout on low verbosity level
- do not create wallet on registry refresh
- change reward balance to optional
- apply interval only to non-running nodes
- do not delete custom bin on `add` cmd
- incorrect release type reference

### Other
- *(versions)* sync versions with latest crates.io vs for nodeman
- *(versions)* sync versions with latest crates.io vs
- use node registry for status
- [**breaking**] output reward balance in `status --json` cmd
- use better banners
- properly use node registry and surface peer ids if they're not
- `remove` cmd operates over all services
- provide `local` subcommand
- clarify client::new description
- *(deps)* bump dependencies

## [0.7.2](https://github.com/joshuef/safe_network/compare/sn-node-manager-v0.7.1...sn-node-manager-v0.7.2) - 2024-03-28

### Other
- updated the following local packages: sn_service_management

## [0.7.1](https://github.com/joshuef/safe_network/compare/sn-node-manager-v0.7.0...sn-node-manager-v0.7.1) - 2024-03-28

### Other
- updated the following local packages: sn_transfers

## [0.7.0](https://github.com/joshuef/safe_network/compare/sn-node-manager-v0.6.1...sn-node-manager-v0.7.0) - 2024-03-27

### Added
- [**breaking**] remove gossip code
- add `--interval` arg to `start` command
- arguments can be used multiple times
- provide `--rpc-port` arg for `add` cmd
- provide `--metrics-port` arg for `add` cmd
- uniform behaviour for all `add` commands

### Fixed
- preclude removed services from ops
- permit removal of manually removed services
- *(manager)* store exclusive reference to service data instead of cloning

### Other
- refresh node registry before commands
- fix wrong command in usage example
- clarify version number usage

## [0.6.1](https://github.com/joshuef/safe_network/compare/sn-node-manager-v0.6.0...sn-node-manager-v0.6.1) - 2024-03-21

### Added
- uniform behaviour for all `add` commands
- *(protocol)* add rpc to set node log level on the fly

### Other
- run `safenodemand` service as root
- upgrade `sn-releases` to new minor version
- remove churn example from node manager

## [0.6.0](https://github.com/joshuef/safe_network/compare/sn-node-manager-v0.5.1...sn-node-manager-v0.6.0) - 2024-03-14

### Added
- *(manager)* add example to cause churn to a running network
- add rpc to fetch status from the daemon

### Fixed
- dont stop spend verification at spend error, generalise spend serde
- *(deps)* add missing service management dep

### Other
- store test utils under a new crate
- reorganise command processing
- *(service)* make the add node naming more explicit
- *(service)* remove the node service restart workaround
- extend `status` cmd for faucet and daemon
- add daemon service behaves uniformly
- correctly run node manager unit tests
- introduce `add_services` module
- move rpc to its own module
- [**breaking**] uniform service management
- new `sn_service_management` crate
- *(release)* sn_transfers-v0.16.3/sn_cli-v0.89.82

## [0.5.1](https://github.com/joshuef/safe_network/compare/sn-node-manager-v0.5.0-alpha.0...sn-node-manager-v0.5.1) - 2024-03-08

### Other
- updated the following local packages: sn_transfers

## [0.4.1](https://github.com/joshuef/safe_network/compare/sn-node-manager-v0.4.0...sn-node-manager-v0.4.1) - 2024-03-06

### Other
- update Cargo.lock dependencies

## [0.4.0](https://github.com/joshuef/safe_network/compare/sn-node-manager-v0.3.11...sn-node-manager-v0.4.0) - 2024-03-05

### Added
- *(manager)* add subcommands for daemon
- *(daemon)* retain peer_id while restarting a safenode service
- *(test)* add option to retain_peer_id for the node's restart rpc cmd
- *(protocol)* add daemon socket addr to node registry
- *(manager)* stop the daemon if it is already running
- *(manager)* add rpc call to restart node service and process
- *(manager)* provide option to start the manager as a daemon
- provide `faucet stop` command
- [**breaking**] provide `faucet start` command
- provide `faucet add` command

### Fixed
- *(test)* provide absolute path for daemon restart test
- *(daemon)* create node service dir while restarting as new peer
- *(daemon)* set the proper safenode path while restarting a service
- *(deps)* don't add unix dep to whole crate
- *(manager)* don't specify user while spawning daemon
- *(manager)* fix sync issue while trying to use trait objects

### Other
- *(release)* sn_protocol-v0.15.0
- get clippy to stop mentioning this
- *(daemon)* rename daemon binary to safenodemand
- *(manager)* add daemon restart test
- *(daemon)* add more context to errors
- *(manager)* removing support for process restarts
- create a `faucet_control` module

## [0.3.11](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.3.10...sn-node-manager-v0.3.11) - 2024-02-23

### Added
- bump alpha versions via releas-plz bump_version script

### Other
- cleanup version in node_manager after experimentation

## [0.3.10](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.3.9...sn-node-manager-v0.3.10) - 2024-02-21

### Other
- update Cargo.lock dependencies

## [0.3.9](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.3.8...sn-node-manager-v0.3.9) - 2024-02-20

### Added
- *(manager)* setup initial bin for safenode mangaer daemon

### Other
- *(deps)* update service manager to the latest version
- *(manager)* move node controls into its own module
- *(manager)* make ServiceControl more generic
- *(manager)* remove panics from the codebase and instead propagate errors
- *(manager)* rename options to be coherent across the lib
- remove unused install file

## [0.3.8](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.3.7...sn-node-manager-v0.3.8) - 2024-02-20

### Other
- *(release)* sn_cli-v0.89.77/sn_client-v0.104.24/sn_faucet-v0.3.76/sn_node-v0.104.32/sn_node_rpc_client-v0.4.63

## [0.3.7](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.3.6...sn-node-manager-v0.3.7) - 2024-02-20

### Fixed
- *(manager)* retry release downloads on failure

## [0.3.6](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.3.5...sn-node-manager-v0.3.6) - 2024-02-20

### Other
- *(release)* sn_cli-v0.89.75/sn_client-v0.104.22/sn_networking-v0.13.25/sn_transfers-v0.15.8/sn_protocol-v0.14.5/sn_faucet-v0.3.74/sn_node-v0.104.30/sn_node_rpc_client-v0.4.61

## [0.3.5](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.3.4...sn-node-manager-v0.3.5) - 2024-02-20

### Other
- *(release)* sn_client-v0.104.20/sn_registers-v0.3.10/sn_node-v0.104.28/sn_cli-v0.89.73/sn_protocol-v0.14.3/sn_faucet-v0.3.72/sn_node_rpc_client-v0.4.59

## [0.3.4](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.3.3...sn-node-manager-v0.3.4) - 2024-02-20

### Other
- *(release)* sn_networking-v0.13.23/sn_node-v0.104.26/sn_client-v0.104.18/sn_node_rpc_client-v0.4.57

## [0.3.3](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.3.2...sn-node-manager-v0.3.3) - 2024-02-19

### Other
- update Cargo.lock dependencies

## [0.3.2](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.3.1...sn-node-manager-v0.3.2) - 2024-02-15

### Other
- update Cargo.lock dependencies

## [0.3.1](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.3.0...sn-node-manager-v0.3.1) - 2024-02-15

### Added
- force and upgrade by url or version

## [0.3.0](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.2.1...sn-node-manager-v0.3.0) - 2024-02-14

### Added
- *(manager)* provide an option to set new env variables during node upgrade
- *(manager)* re-use the same env variables during the upgrade process
- *(manager)* [**breaking**] store the env variables inside the NodeRegistry
- *(manager)* provide enviroment variable to the service definition file during add

### Other
- *(docs)* update based on comments

## [0.2.1](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.2.0...sn-node-manager-v0.2.1) - 2024-02-14

### Other
- updated the following local packages: sn_protocol

## [0.2.0](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.62...sn-node-manager-v0.2.0) - 2024-02-13

### Added
- *(protocol)* include local flag inside registry's Node struct
- *(sn_protocol)* [**breaking**] store the bootstrap peers inside the NodeRegistry

### Fixed
- *(manager)* restart nodes with the same safenode port

### Other
- *(manager)* move bootstrap_peers store step inside add fn
- *(protocol)* [**breaking**] make node dirs not optional

## [0.1.62](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.61...sn-node-manager-v0.1.62) - 2024-02-13

### Other
- *(release)* sn_cli-v0.89.64/sn_client-v0.104.9/sn_transfers-v0.15.4/sn_networking-v0.13.14/sn_protocol-v0.12.7/sn_faucet-v0.3.64/sn_node-v0.104.16/sn_node_rpc_client-v0.4.49

## [0.1.61](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.60...sn-node-manager-v0.1.61) - 2024-02-12

### Other
- *(release)* sn_node-v0.104.15/sn_node_rpc_client-v0.4.48

## [0.1.60](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.59...sn-node-manager-v0.1.60) - 2024-02-12

### Other
- update Cargo.lock dependencies

## [0.1.59](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.58...sn-node-manager-v0.1.59) - 2024-02-12

### Other
- *(release)* sn_cli-v0.89.62/sn_client-v0.104.6/sn_node-v0.104.11/sn_faucet-v0.3.62/sn_node_rpc_client-v0.4.45

## [0.1.58](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.57...sn-node-manager-v0.1.58) - 2024-02-12

### Fixed
- apply suspicious_open_options from clippy

## [0.1.57](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.56...sn-node-manager-v0.1.57) - 2024-02-09

### Other
- updated the following local packages: sn_node_rpc_client

## [0.1.56](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.55...sn-node-manager-v0.1.56) - 2024-02-08

### Other
- update dependencies

## [0.1.55](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.54...sn-node-manager-v0.1.55) - 2024-02-08

### Other
- update dependencies

## [0.1.54](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.53...sn-node-manager-v0.1.54) - 2024-02-08

### Other
- update dependencies

## [0.1.53](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.52...sn-node-manager-v0.1.53) - 2024-02-08

### Other
- update dependencies

## [0.1.52](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.51...sn-node-manager-v0.1.52) - 2024-02-08

### Other
- update dependencies

## [0.1.51](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.50...sn-node-manager-v0.1.51) - 2024-02-08

### Other
- improvements from dev feedback

## [0.1.50](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.49...sn-node-manager-v0.1.50) - 2024-02-07

### Other
- update dependencies

## [0.1.49](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.48...sn-node-manager-v0.1.49) - 2024-02-06

### Other
- update dependencies

## [0.1.48](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.47...sn-node-manager-v0.1.48) - 2024-02-06

### Other
- update dependencies

## [0.1.47](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.46...sn-node-manager-v0.1.47) - 2024-02-06

### Other
- update dependencies

## [0.1.46](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.45...sn-node-manager-v0.1.46) - 2024-02-05

### Other
- update dependencies

## [0.1.45](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.44...sn-node-manager-v0.1.45) - 2024-02-05

### Other
- update dependencies

## [0.1.44](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.43...sn-node-manager-v0.1.44) - 2024-02-05

### Other
- update dependencies

## [0.1.43](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.42...sn-node-manager-v0.1.43) - 2024-02-05

### Other
- update dependencies

## [0.1.42](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.41...sn-node-manager-v0.1.42) - 2024-02-05

### Other
- update dependencies

## [0.1.41](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.40...sn-node-manager-v0.1.41) - 2024-02-05

### Fixed
- node manager `status` permissions error

## [0.1.40](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.39...sn-node-manager-v0.1.40) - 2024-02-02

### Fixed
- *(manager)* set the entire service file details for linux
- *(manager)* set safenode service KillMode to fix restarts

## [0.1.39](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.38...sn-node-manager-v0.1.39) - 2024-02-02

### Other
- update dependencies

## [0.1.38](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.37...sn-node-manager-v0.1.38) - 2024-02-02

### Other
- update dependencies

## [0.1.37](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.36...sn-node-manager-v0.1.37) - 2024-02-01

### Other
- update dependencies

## [0.1.36](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.35...sn-node-manager-v0.1.36) - 2024-02-01

### Other
- update dependencies

## [0.1.35](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.34...sn-node-manager-v0.1.35) - 2024-02-01

### Other
- update dependencies

## [0.1.34](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.33...sn-node-manager-v0.1.34) - 2024-01-31

### Added
- provide `--build` flag for commands

### Other
- download binary once for `add` command
- misc clean up for local testnets

## [0.1.33](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.32...sn-node-manager-v0.1.33) - 2024-01-31

### Other
- update dependencies

## [0.1.32](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.31...sn-node-manager-v0.1.32) - 2024-01-31

### Other
- update dependencies

## [0.1.31](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.30...sn-node-manager-v0.1.31) - 2024-01-30

### Other
- update dependencies

## [0.1.30](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.29...sn-node-manager-v0.1.30) - 2024-01-30

### Other
- update dependencies

## [0.1.29](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.28...sn-node-manager-v0.1.29) - 2024-01-30

### Other
- update dependencies

## [0.1.28](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.27...sn-node-manager-v0.1.28) - 2024-01-30

### Other
- update dependencies

## [0.1.27](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.26...sn-node-manager-v0.1.27) - 2024-01-30

### Other
- *(manager)* provide rpc address instead of rpc port

## [0.1.26](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.25...sn-node-manager-v0.1.26) - 2024-01-29

### Other
- *(manager)* make VerbosityLevel a public type

## [0.1.25](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.24...sn-node-manager-v0.1.25) - 2024-01-29

### Other
- provide verbosity level
- improve error handling for `start` command
- improve error handling for `add` command
- version and url arguments conflict

## [0.1.24](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.23...sn-node-manager-v0.1.24) - 2024-01-29

### Other
- update dependencies

## [0.1.23](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.22...sn-node-manager-v0.1.23) - 2024-01-26

### Other
- update dependencies

## [0.1.22](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.21...sn-node-manager-v0.1.22) - 2024-01-25

### Other
- update dependencies

## [0.1.21](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.20...sn-node-manager-v0.1.21) - 2024-01-25

### Other
- update dependencies

## [0.1.20](https://github.com/maidsafe/safe_network/compare/sn-node-manager-v0.1.19...sn-node-manager-v0.1.20) - 2024-01-25

### Fixed
- *(manager)* increase port unbinding time

### Other
- rename sn_node_manager crate
- *(manager)* rename node manager crate

## [0.1.19](https://github.com/maidsafe/sn-node-manager/compare/v0.1.18...v0.1.19) - 2024-01-23

### Fixed
- add delay to make sure we drop the socket

### Other
- force skip validation

## [0.1.18](https://github.com/maidsafe/sn-node-manager/compare/v0.1.17...v0.1.18) - 2024-01-22

### Added
- provide `faucet` command
- `status` command enhancements
- provide `--local` flag for `add`

### Other
- fixup after rebase
- provide script for local network
- additional info in `status` cmd

## [0.1.17](https://github.com/maidsafe/sn-node-manager/compare/v0.1.16...v0.1.17) - 2024-01-18

### Added
- add quic/tcp features and set quic as default

## [0.1.16](https://github.com/maidsafe/sn-node-manager/compare/v0.1.15...v0.1.16) - 2024-01-16

### Other
- tidy peer management for `join` command

## [0.1.15](https://github.com/maidsafe/sn-node-manager/compare/v0.1.14...v0.1.15) - 2024-01-15

### Other
- manually parse environment variable

## [0.1.14](https://github.com/maidsafe/sn-node-manager/compare/v0.1.13...v0.1.14) - 2024-01-12

### Added
- apply `--first` argument to added service

## [0.1.13](https://github.com/maidsafe/sn-node-manager/compare/v0.1.12...v0.1.13) - 2024-01-10

### Fixed
- apply to correct argument

## [0.1.12](https://github.com/maidsafe/sn-node-manager/compare/v0.1.11...v0.1.12) - 2024-01-09

### Other
- use `--first` arg for genesis node

## [0.1.11](https://github.com/maidsafe/sn-node-manager/compare/v0.1.10...v0.1.11) - 2023-12-21

### Added
- download binaries in absence of paths

## [0.1.10](https://github.com/maidsafe/sn-node-manager/compare/v0.1.9...v0.1.10) - 2023-12-19

### Added
- provide `run` command

## [0.1.9](https://github.com/maidsafe/sn-node-manager/compare/v0.1.8...v0.1.9) - 2023-12-14

### Added
- custom port arguments for `add` command

## [0.1.8](https://github.com/maidsafe/sn-node-manager/compare/v0.1.7...v0.1.8) - 2023-12-13

### Other
- remove network contacts from peer acquisition

## [0.1.7](https://github.com/maidsafe/sn-node-manager/compare/v0.1.6...v0.1.7) - 2023-12-13

### Added
- provide `--url` argument for `add` command

## [0.1.6](https://github.com/maidsafe/sn-node-manager/compare/v0.1.5...v0.1.6) - 2023-12-12

### Fixed
- accommodate service restarts in `status` cmd

## [0.1.5](https://github.com/maidsafe/sn-node-manager/compare/v0.1.4...v0.1.5) - 2023-12-08

### Added
- provide `upgrade` command
- each service instance to use its own binary

## [0.1.4](https://github.com/maidsafe/sn-node-manager/compare/v0.1.3...v0.1.4) - 2023-12-05

### Other
- upload 'latest' version to S3

## [0.1.3](https://github.com/maidsafe/sn-node-manager/compare/v0.1.2...v0.1.3) - 2023-12-05

### Added
- provide `remove` command

## [0.1.2](https://github.com/maidsafe/sn-node-manager/compare/v0.1.1...v0.1.2) - 2023-12-05

### Added
- provide `--peer` argument

### Other
- rename `install` command to `add`

## [0.1.1](https://github.com/maidsafe/sn-node-manager/compare/v0.1.0...v0.1.1) - 2023-11-29

### Other
- improve docs for `start` and `stop` commands

## [0.1.0](https://github.com/maidsafe/sn-node-manager/releases/tag/v0.1.0) - 2023-11-29

### Added
- provide `status` command
- provide `stop` command
- provide `start` command
- provide `install` command

### Other
- release process and licensing
- extend the e2e test for new commands
- reference `sn_node_rpc_client` crate
- specify root and log dirs at install time
- provide initial integration tests
- Initial commit
