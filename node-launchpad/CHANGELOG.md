# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.4](https://github.com/joshuef/safe_network/compare/node-launchpad-v0.3.3...node-launchpad-v0.3.4) - 2024-06-04

### Added
- *(launchpad)* limit node count to 50
- provide `--autostart` flag for `add` command
- configure winsw in `node-launchpad`
- *(launchpad)* use nat detection server to determine the nat status
- *(launchpad)* update the start stop node popup
- *(launchpad)* keep track of the nodes to start
- *(launchpad)* update manage nodes ui
- *(launchpad)* implement help pop up
- *(launchpad)* revamp the beta programme flow
- *(launchpad)* update footer to include two lines of commands
- *(launchpad)* setup the basic device status table
- *(launchpad)* set a new header for the home scene
- *(launchpad)* set new discord id on change
- *(manager)* implement nat detection during safenode add
- *(launchpad)* provide safenode path for testing
- *(manager)* maintain n running nodes
- *(auditor)* add new beta participants via endpoint
- *(launchpad)* accept peers args
- supply discord username on launchpad
- provide `--owner` arg for `add` cmd
- *(nodeman)* add LogFormat as a startup arg for nodes
- *(node-launchpad)* discord name widget styling
- *(node-launchpad)* tweaks on resource allocation widget
- *(launchpad)* initial automatic resource allocation logic
- *(launchpad)* allow users to input disk space to allocate
- *(launchpad)* store discord username to disk
- *(launchpad)* use escape to exit input screen and restore old value
- *(launchpad)* have customizable footer
- *(launchpad)* add discord username scene
- *(launchpad)* remove separate ai launcher bin references
- *(launchpad)* ensure start mac launchapd with sudo only if not set

### Fixed
- *(launchpad)* pressing enter should start nodes
- *(launchpad)* make the bg colors work better
- *(manager)* update nat detection exit code
- retain options on upgrade and prevent dup ports
- *(launchpad)* check if component is active before handling events
- *(launchpad)* prevent mac opening with sudo
- use fixed size popups
- *(launchpad)* prevent loops from terminal/sudo relaunching
- *(launchpad)* do not try to run sudo twice

### Other
- *(release)* sn_auditor-v0.1.21/sn_client-v0.107.4/sn_peers_acquisition-v0.3.2/sn_cli-v0.93.3/sn_faucet-v0.4.23/sn_node-v0.107.3/node-launchpad-v0.3.3/sn-node-manager-v0.9.3/sn_node_rpc_client-v0.6.21
- *(release)* sn_client-v0.107.3/sn_transfers-v0.18.4/sn_cli-v0.93.2/sn_node-v0.107.2/node-launchpad-v0.3.2/sn-node-manager-v0.9.2/sn_auditor-v0.1.20/sn_networking-v0.16.2/sn_protocol-v0.17.2/sn_faucet-v0.4.22/sn_service_management-v0.3.3/sn_node_rpc_client-v0.6.20
- *(release)* sn_faucet-v0.4.21/sn_service_management-v0.3.2/sn_transfers-v0.18.3/sn_auditor-v0.1.19/sn_client-v0.107.2/sn_networking-v0.16.1/sn_cli-v0.93.1/sn_node-v0.107.1/node-launchpad-v0.3.1/sn-node-manager-v0.9.1/sn_peers_acquisition-v0.3.1
- bump versions to enable re-release with env vars at compilation
- *(release)* sn_auditor-v0.1.18/sn_client-v0.107.0/sn_networking-v0.16.0/sn_protocol-v0.17.0/sn_transfers-v0.18.2/sn_peers_acquisition-v0.3.0/sn_cli-v0.93.0/sn_faucet-v0.4.20/sn_metrics-v0.1.8/sn_node-v0.107.0/sn_service_management-v0.3.1/node-launchpad-v0.3.0/sn-node-manager-v0.9.0/sn_node_rpc_client-v0.6.19
- *(launchpad)* typo fix, use program
- *(launchpad)* fixes for white terminal theme
- *(launchpad)* disable unused stats
- *(launchpad)* disable start stop if node count not set
- *(launchpad)* use the correct styling throughout
- *(launchpad)* update scene variant name to BetaProgramme
- *(launchpad)* removed the splash screen on discord id submition
- *(launchpad)* set new color scheme for home
- *(launchpad)* update node status box
- *(manager)* move nat detection out of add subcommand
- *(release)* sn_auditor-v0.1.17/sn_client-v0.106.3/sn_networking-v0.15.3/sn_transfers-v0.18.1/sn_logging-v0.2.27/sn_cli-v0.92.0/sn_faucet-v0.4.19/sn_node-v0.106.5/sn_service_management-v0.3.0/node-launchpad-v0.2.0/sn-node-manager-v0.8.0/sn_protocol-v0.16.7/sn_node_rpc_client-v0.6.18
- update sn-releases
- update based on comment
- *(release)* sn_auditor-v0.1.16/sn_cli-v0.91.4/sn_faucet-v0.4.18/sn_metrics-v0.1.7/sn_node-v0.106.4/sn_service_management-v0.2.8/node-launchpad-v0.1.5/sn-node-manager-v0.7.7/sn_node_rpc_client-v0.6.17
- check we are in terminal before creating one
- *(release)* node-launchpad-v0.1.4
- use published versions of deps
- *(release)* node-launchpad-v0.1.3/sn-node-manager-v0.7.6
- *(release)* sn_auditor-v0.1.15/sn_cli-v0.91.3/sn_faucet-v0.4.17/sn_metrics-v0.1.6/sn_node-v0.106.3/sn_service_management-v0.2.7/node-launchpad-v0.1.2/sn_node_rpc_client-v0.6.16
- *(launchpad)* removing redudnat for loops
- move helper text inside popup
- change trigger resource allocation input box keybind
- *(launchpad)* highlight the table in green if we're currently running
- *(launchpad)* add more alternative keybinds
- change terminal launch behaviour
- use consistent border styles
- *(launchpad)* use safe data dir to store configs
- *(release)* sn_auditor-v0.1.13/sn_client-v0.106.1/sn_networking-v0.15.1/sn_protocol-v0.16.6/sn_cli-v0.91.1/sn_faucet-v0.4.15/sn_node-v0.106.1/node-launchpad-v0.1.1/sn_node_rpc_client-v0.6.14/sn_peers_acquisition-v0.2.12/sn_service_management-v0.2.6
- *(release)* sn_auditor-v0.1.12/sn_client-v0.106.0/sn_networking-v0.15.0/sn_transfers-v0.18.0/sn_peers_acquisition-v0.2.11/sn_logging-v0.2.26/sn_cli-v0.91.0/sn_faucet-v0.4.14/sn_metrics-v0.1.5/sn_node-v0.106.0/sn_service_management-v0.2.5/test_utils-v0.4.1/node-launchpad-v/sn-node-manager-v0.7.5/sn_node_rpc_client-v0.6.13/token_supplies-v0.1.48/sn_protocol-v0.16.5
- *(versions)* sync versions with latest crates.io vs for nodeman
- *(versions)* sync versions with latest crates.io vs
- rename sn_node_launchpad -> node-launchpad
- rename `node-launchpad` crate to `sn_node_launchpad`
- rebased and removed custom rustfmt
- *(tui)* rename crate

## [0.3.3](https://github.com/joshuef/safe_network/compare/node-launchpad-v0.3.2...node-launchpad-v0.3.3) - 2024-06-04

### Added
- *(launchpad)* limit node count to 50
- *(faucet_server)* download and upload gutenberger book part by part

### Fixed
- *(launchpad)* pressing enter should start nodes

### Other
- *(release)* sn_client-v0.107.3/sn_transfers-v0.18.4/sn_cli-v0.93.2/sn_node-v0.107.2/node-launchpad-v0.3.2/sn-node-manager-v0.9.2/sn_auditor-v0.1.20/sn_networking-v0.16.2/sn_protocol-v0.17.2/sn_faucet-v0.4.22/sn_service_management-v0.3.3/sn_node_rpc_client-v0.6.20
- reduce dag recrawl interval

## [0.3.2](https://github.com/joshuef/safe_network/compare/node-launchpad-v0.3.1...node-launchpad-v0.3.2) - 2024-06-03

### Added
- provide `--autostart` flag for `add` command
- configure winsw in `node-launchpad`
- *(launchpad)* use nat detection server to determine the nat status
- *(launchpad)* update the start stop node popup
- *(launchpad)* keep track of the nodes to start
- *(launchpad)* update manage nodes ui
- *(launchpad)* implement help pop up
- *(launchpad)* revamp the beta programme flow
- *(launchpad)* update footer to include two lines of commands
- *(launchpad)* setup the basic device status table
- *(launchpad)* set a new header for the home scene
- *(launchpad)* set new discord id on change
- *(manager)* implement nat detection during safenode add
- *(launchpad)* provide safenode path for testing
- *(manager)* maintain n running nodes
- *(auditor)* add new beta participants via endpoint
- *(launchpad)* accept peers args
- supply discord username on launchpad
- provide `--owner` arg for `add` cmd
- *(nodeman)* add LogFormat as a startup arg for nodes
- *(node-launchpad)* discord name widget styling
- *(node-launchpad)* tweaks on resource allocation widget
- *(launchpad)* initial automatic resource allocation logic
- *(launchpad)* allow users to input disk space to allocate
- *(launchpad)* store discord username to disk
- *(launchpad)* use escape to exit input screen and restore old value
- *(launchpad)* have customizable footer
- *(launchpad)* add discord username scene
- *(launchpad)* remove separate ai launcher bin references
- *(launchpad)* ensure start mac launchapd with sudo only if not set

### Fixed
- *(launchpad)* make the bg colors work better
- *(manager)* update nat detection exit code
- retain options on upgrade and prevent dup ports
- *(launchpad)* check if component is active before handling events
- *(launchpad)* prevent mac opening with sudo
- use fixed size popups
- *(launchpad)* prevent loops from terminal/sudo relaunching
- *(launchpad)* do not try to run sudo twice

### Other
- *(release)* sn_faucet-v0.4.21/sn_service_management-v0.3.2/sn_transfers-v0.18.3/sn_auditor-v0.1.19/sn_client-v0.107.2/sn_networking-v0.16.1/sn_cli-v0.93.1/sn_node-v0.107.1/node-launchpad-v0.3.1/sn-node-manager-v0.9.1/sn_peers_acquisition-v0.3.1
- bump versions to enable re-release with env vars at compilation
- *(release)* sn_auditor-v0.1.18/sn_client-v0.107.0/sn_networking-v0.16.0/sn_protocol-v0.17.0/sn_transfers-v0.18.2/sn_peers_acquisition-v0.3.0/sn_cli-v0.93.0/sn_faucet-v0.4.20/sn_metrics-v0.1.8/sn_node-v0.107.0/sn_service_management-v0.3.1/node-launchpad-v0.3.0/sn-node-manager-v0.9.0/sn_node_rpc_client-v0.6.19
- *(launchpad)* typo fix, use program
- *(launchpad)* fixes for white terminal theme
- *(launchpad)* disable unused stats
- *(launchpad)* disable start stop if node count not set
- *(launchpad)* use the correct styling throughout
- *(launchpad)* update scene variant name to BetaProgramme
- *(launchpad)* removed the splash screen on discord id submition
- *(launchpad)* set new color scheme for home
- *(launchpad)* update node status box
- *(manager)* move nat detection out of add subcommand
- *(release)* sn_auditor-v0.1.17/sn_client-v0.106.3/sn_networking-v0.15.3/sn_transfers-v0.18.1/sn_logging-v0.2.27/sn_cli-v0.92.0/sn_faucet-v0.4.19/sn_node-v0.106.5/sn_service_management-v0.3.0/node-launchpad-v0.2.0/sn-node-manager-v0.8.0/sn_protocol-v0.16.7/sn_node_rpc_client-v0.6.18
- update sn-releases
- update based on comment
- *(release)* sn_auditor-v0.1.16/sn_cli-v0.91.4/sn_faucet-v0.4.18/sn_metrics-v0.1.7/sn_node-v0.106.4/sn_service_management-v0.2.8/node-launchpad-v0.1.5/sn-node-manager-v0.7.7/sn_node_rpc_client-v0.6.17
- check we are in terminal before creating one
- *(release)* node-launchpad-v0.1.4
- use published versions of deps
- *(release)* node-launchpad-v0.1.3/sn-node-manager-v0.7.6
- *(release)* sn_auditor-v0.1.15/sn_cli-v0.91.3/sn_faucet-v0.4.17/sn_metrics-v0.1.6/sn_node-v0.106.3/sn_service_management-v0.2.7/node-launchpad-v0.1.2/sn_node_rpc_client-v0.6.16
- *(launchpad)* removing redudnat for loops
- move helper text inside popup
- change trigger resource allocation input box keybind
- *(launchpad)* highlight the table in green if we're currently running
- *(launchpad)* add more alternative keybinds
- change terminal launch behaviour
- use consistent border styles
- *(launchpad)* use safe data dir to store configs
- *(release)* sn_auditor-v0.1.13/sn_client-v0.106.1/sn_networking-v0.15.1/sn_protocol-v0.16.6/sn_cli-v0.91.1/sn_faucet-v0.4.15/sn_node-v0.106.1/node-launchpad-v0.1.1/sn_node_rpc_client-v0.6.14/sn_peers_acquisition-v0.2.12/sn_service_management-v0.2.6
- *(release)* sn_auditor-v0.1.12/sn_client-v0.106.0/sn_networking-v0.15.0/sn_transfers-v0.18.0/sn_peers_acquisition-v0.2.11/sn_logging-v0.2.26/sn_cli-v0.91.0/sn_faucet-v0.4.14/sn_metrics-v0.1.5/sn_node-v0.106.0/sn_service_management-v0.2.5/test_utils-v0.4.1/node-launchpad-v/sn-node-manager-v0.7.5/sn_node_rpc_client-v0.6.13/token_supplies-v0.1.48/sn_protocol-v0.16.5
- *(versions)* sync versions with latest crates.io vs for nodeman
- *(versions)* sync versions with latest crates.io vs
- rename sn_node_launchpad -> node-launchpad
- rename `node-launchpad` crate to `sn_node_launchpad`
- rebased and removed custom rustfmt
- *(tui)* rename crate

## [0.3.1](https://github.com/joshuef/safe_network/compare/node-launchpad-v0.3.0...node-launchpad-v0.3.1) - 2024-06-03

### Other
- bump versions to enable re-release with env vars at compilation

## [0.3.0](https://github.com/joshuef/safe_network/compare/node-launchpad-v0.2.0...node-launchpad-v0.3.0) - 2024-06-03

### Added
- provide `--autostart` flag for `add` command
- configure winsw in `node-launchpad`
- *(launchpad)* use nat detection server to determine the nat status
- *(launchpad)* update the start stop node popup
- *(launchpad)* keep track of the nodes to start
- *(launchpad)* update manage nodes ui
- *(launchpad)* implement help pop up
- *(launchpad)* revamp the beta programme flow
- *(launchpad)* update footer to include two lines of commands
- *(launchpad)* setup the basic device status table
- *(launchpad)* set a new header for the home scene
- *(launchpad)* set new discord id on change
- *(manager)* implement nat detection during safenode add
- *(node)* make payment forward optional
- *(network)* [**breaking**] move network versioning away from sn_protocol
- configure winsw in node manager
- *(node_manager)* add unit tests and modify docs

### Fixed
- *(launchpad)* make the bg colors work better
- *(manager)* update nat detection exit code

### Other
- *(launchpad)* typo fix, use program
- *(launchpad)* fixes for white terminal theme
- *(launchpad)* disable unused stats
- *(launchpad)* disable start stop if node count not set
- *(launchpad)* use the correct styling throughout
- *(launchpad)* update scene variant name to BetaProgramme
- *(launchpad)* removed the splash screen on discord id submition
- *(launchpad)* set new color scheme for home
- *(launchpad)* update node status box
- *(manager)* move nat detection out of add subcommand
- *(release)* sn_auditor-v0.1.17/sn_client-v0.106.3/sn_networking-v0.15.3/sn_transfers-v0.18.1/sn_logging-v0.2.27/sn_cli-v0.92.0/sn_faucet-v0.4.19/sn_node-v0.106.5/sn_service_management-v0.3.0/node-launchpad-v0.2.0/sn-node-manager-v0.8.0/sn_protocol-v0.16.7/sn_node_rpc_client-v0.6.18
- use new version of `service-manager` crate
- update NodeInfo struct inside the tests

## [0.2.0](https://github.com/joshuef/safe_network/compare/node-launchpad-v0.1.5...node-launchpad-v0.2.0) - 2024-05-24

### Added
- *(launchpad)* provide safenode path for testing
- *(manager)* maintain n running nodes
- *(auditor)* add new beta participants via endpoint
- *(launchpad)* accept peers args
- supply discord username on launchpad
- provide `--owner` arg for `add` cmd
- *(nodeman)* add LogFormat as a startup arg for nodes
- *(node-launchpad)* discord name widget styling
- *(node-launchpad)* tweaks on resource allocation widget
- *(launchpad)* initial automatic resource allocation logic
- *(launchpad)* allow users to input disk space to allocate
- *(launchpad)* store discord username to disk
- *(launchpad)* use escape to exit input screen and restore old value
- *(launchpad)* have customizable footer
- *(launchpad)* add discord username scene
- *(launchpad)* remove separate ai launcher bin references
- *(launchpad)* ensure start mac launchapd with sudo only if not set
- use different key for payment forward
- hide genesis keypair
- *(node)* periodically forward reward to specific address
- spend reason enum and sized cipher
- *(network)* add --upnp flag to node
- spend shows the purposes of outputs created for
- *(node)* make spend and cash_note reason field configurable
- *(relay)* remove autonat and enable hole punching manually
- *(relay)* impl RelayManager to perform circuit relay when behind NAT
- *(node)* notify peer it is now considered as BAD
- *(networking)* shift to use ilog2 bucket distance for close data calcs
- unit testing dag, double spend poisoning tweaks
- report protocol mismatch error
- *(node_manager)* pass beta encryption sk to the auditor
- provide `local status` command
- *(node_manager)* add auditor support
- provide `--upnp` flag for `add` command
- *(audit)* collect payment forward statistics
- run safenode services in user mode
- provide `autonomi-launcher` binary
- *(manager)* reuse downloaded binaries
- *(launchpad)* remove nodes
- *(tui)* adding services
- [**breaking**] provide `--home-network` arg for `add` cmd
- provide `--interval` arg for `upgrade` cmd
- provide `--path` arg for `upgrade` cmd
- rpc restart command
- provide `reset` command
- provide `balance` command
- make `--peer` argument optional
- distinguish failure to start during upgrade

### Fixed
- retain options on upgrade and prevent dup ports
- *(launchpad)* check if component is active before handling events
- *(launchpad)* prevent mac opening with sudo
- use fixed size popups
- *(launchpad)* prevent loops from terminal/sudo relaunching
- *(launchpad)* do not try to run sudo twice
- *(node)* notify fetch completion earlier to avoid being skipped
- create faucet via account load or generation
- more test and cli fixes
- update calls to HotWallet::load
- do not add reported external addressese if we are behind home network
- *(node)* notify replication_fetcher of early completion
- *(node)* not send out replication when failed read from local
- avoid adding mixed type addresses into RT
- *(manager)* download again if cached archive is corrupted
- check node registry exists before deleting it
- *(manager)* do not print to stdout on low verbosity level
- do not create wallet on registry refresh
- change reward balance to optional
- apply interval only to non-running nodes
- do not delete custom bin on `add` cmd
- incorrect release type reference
- use correct release type in upgrade process

### Other
- update sn-releases
- update based on comment
- *(release)* sn_auditor-v0.1.16/sn_cli-v0.91.4/sn_faucet-v0.4.18/sn_metrics-v0.1.7/sn_node-v0.106.4/sn_service_management-v0.2.8/node-launchpad-v0.1.5/sn-node-manager-v0.7.7/sn_node_rpc_client-v0.6.17
- check we are in terminal before creating one
- *(release)* node-launchpad-v0.1.4
- use published versions of deps
- *(release)* node-launchpad-v0.1.3/sn-node-manager-v0.7.6
- *(release)* sn_auditor-v0.1.15/sn_cli-v0.91.3/sn_faucet-v0.4.17/sn_metrics-v0.1.6/sn_node-v0.106.3/sn_service_management-v0.2.7/node-launchpad-v0.1.2/sn_node_rpc_client-v0.6.16
- *(launchpad)* removing redudnat for loops
- move helper text inside popup
- change trigger resource allocation input box keybind
- *(launchpad)* highlight the table in green if we're currently running
- *(launchpad)* add more alternative keybinds
- change terminal launch behaviour
- use consistent border styles
- *(launchpad)* use safe data dir to store configs
- *(release)* sn_auditor-v0.1.13/sn_client-v0.106.1/sn_networking-v0.15.1/sn_protocol-v0.16.6/sn_cli-v0.91.1/sn_faucet-v0.4.15/sn_node-v0.106.1/node-launchpad-v0.1.1/sn_node_rpc_client-v0.6.14/sn_peers_acquisition-v0.2.12/sn_service_management-v0.2.6
- *(release)* sn_auditor-v0.1.12/sn_client-v0.106.0/sn_networking-v0.15.0/sn_transfers-v0.18.0/sn_peers_acquisition-v0.2.11/sn_logging-v0.2.26/sn_cli-v0.91.0/sn_faucet-v0.4.14/sn_metrics-v0.1.5/sn_node-v0.106.0/sn_service_management-v0.2.5/test_utils-v0.4.1/node-launchpad-v/sn-node-manager-v0.7.5/sn_node_rpc_client-v0.6.13/token_supplies-v0.1.48/sn_protocol-v0.16.5
- *(versions)* sync versions with latest crates.io vs for nodeman
- *(versions)* sync versions with latest crates.io vs
- rename sn_node_launchpad -> node-launchpad
- rename `node-launchpad` crate to `sn_node_launchpad`
- rebased and removed custom rustfmt
- *(tui)* rename crate
- *(node)* log node owner
- make open metrics feature default but without starting it by default
- *(refactor)* stabilise node size to 4k records,
- resolve errors after reverts
- Revert "feat(node): make spend and cash_note reason field configurable"
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
- *(release)* sn_auditor-v0.1.7/sn_client-v0.105.3/sn_networking-v0.14.4/sn_protocol-v0.16.3/sn_build_info-v0.1.7/sn_transfers-v0.17.2/sn_peers_acquisition-v0.2.10/sn_cli-v0.90.4/sn_faucet-v0.4.9/sn_metrics-v0.1.4/sn_node-v0.105.6/sn_service_management-v0.2.4/sn-node-manager-v0.7.4/sn_node_rpc_client-v0.6.8/token_supplies-v0.1.47
- *(deps)* bump dependencies
- *(node)* pass entire QuotingMetrics into calculate_cost_for_records
- enable node man integration tests
- use owners on memcheck workflow local network
- reconfigure local network owner args
- *(nodemanager)* upgrade_should_retain_the_log_format_flag
- use helper function to print banners
- use const for default user or owner
- update cli and readme for user-mode services
- upgrade service manager crate
- use node registry for status
- [**breaking**] output reward balance in `status --json` cmd
- use better banners
- properly use node registry and surface peer ids if they're not
- `remove` cmd operates over all services
- provide `local` subcommand

## [0.1.5](https://github.com/maidsafe/safe_network/compare/node-launchpad-v0.1.4...node-launchpad-v0.1.5) - 2024-05-20

### Added
- *(node_manager)* add auditor support
- provide `--upnp` flag for `add` command

### Fixed
- retain options on upgrade and prevent dup ports

### Other
- use published versions of deps
- update Cargo.lock dependencies
- use helper function to print banners

## [0.1.4](https://github.com/maidsafe/safe_network/compare/node-launchpad-v0.1.3...node-launchpad-v0.1.4) - 2024-05-17

### Added
- *(node-launchpad)* discord name widget styling
- *(node-launchpad)* tweaks on resource allocation widget

## [0.1.3](https://github.com/maidsafe/safe_network/compare/node-launchpad-v0.1.2...node-launchpad-v0.1.3) - 2024-05-15

### Added
- *(launchpad)* initial automatic resource allocation logic
- run safenode services in user mode

### Other
- *(release)* sn_auditor-v0.1.15/sn_cli-v0.91.3/sn_faucet-v0.4.17/sn_metrics-v0.1.6/sn_node-v0.106.3/sn_service_management-v0.2.7/node-launchpad-v0.1.2/sn_node_rpc_client-v0.6.16
- change terminal launch behaviour
- update cli and readme for user-mode services
- upgrade service manager crate
- *(release)* sn_auditor-v0.1.13/sn_client-v0.106.1/sn_networking-v0.15.1/sn_protocol-v0.16.6/sn_cli-v0.91.1/sn_faucet-v0.4.15/sn_node-v0.106.1/node-launchpad-v0.1.1/sn_node_rpc_client-v0.6.14/sn_peers_acquisition-v0.2.12/sn_service_management-v0.2.6

## [0.1.2](https://github.com/maidsafe/safe_network/compare/node-launchpad-v0.1.1...node-launchpad-v0.1.2) - 2024-05-15

### Added
- *(launchpad)* initial automatic resource allocation logic
- *(launchpad)* allow users to input disk space to allocate
- *(launchpad)* store discord username to disk
- *(launchpad)* use escape to exit input screen and restore old value
- *(launchpad)* have customizable footer
- *(launchpad)* add discord username scene

### Fixed
- *(launchpad)* check if component is active before handling events
- *(launchpad)* prevent mac opening with sudo
- *(launchpad)* prevent loops from terminal/sudo relaunching
- use fixed size popups

### Other
- *(launchpad)* removing redudnat for loops
- move helper text inside popup
- change trigger resource allocation input box keybind
- *(launchpad)* highlight the table in green if we're currently running
- *(launchpad)* add more alternative keybinds
- change terminal launch behaviour
- use consistent border styles
- *(launchpad)* use safe data dir to store configs

## [0.1.1](https://github.com/maidsafe/safe_network/compare/node-launchpad-v0.1.0...node-launchpad-v0.1.1) - 2024-05-08

### Other
- update Cargo.lock dependencies

## [0.1.0](https://github.com/maidsafe/safe_network/releases/tag/node-launchpad-v0.1.0) - 2024-05-07

### Added
- *(launchpad)* remove separate ai launcher bin references
- *(launchpad)* ensure start mac launchapd with sudo only if not set

### Fixed
- *(launchpad)* do not try to run sudo twice

### Other
- *(versions)* sync versions with latest crates.io vs for nodeman
- *(versions)* sync versions with latest crates.io vs
- rename sn_node_launchpad -> node-launchpad
- rename `node-launchpad` crate to `sn_node_launchpad`
- rebased and removed custom rustfmt
- *(tui)* rename crate
