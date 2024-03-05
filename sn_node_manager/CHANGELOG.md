# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
- bump alpha versions via releas-plz bump_version script
- *(manager)* setup initial bin for safenode mangaer daemon
- force and upgrade by url or version
- *(manager)* provide an option to set new env variables during node upgrade
- *(manager)* re-use the same env variables during the upgrade process
- *(manager)* [**breaking**] store the env variables inside the NodeRegistry
- *(manager)* provide enviroment variable to the service definition file during add
- *(protocol)* include local flag inside registry's Node struct
- *(sn_protocol)* [**breaking**] store the bootstrap peers inside the NodeRegistry
- provide `--build` flag for commands

### Fixed
- *(test)* provide absolute path for daemon restart test
- *(daemon)* create node service dir while restarting as new peer
- *(daemon)* set the proper safenode path while restarting a service
- *(deps)* don't add unix dep to whole crate
- *(manager)* don't specify user while spawning daemon
- *(manager)* fix sync issue while trying to use trait objects
- *(manager)* retry release downloads on failure
- *(manager)* restart nodes with the same safenode port
- apply suspicious_open_options from clippy
- node manager `status` permissions error
- *(manager)* set the entire service file details for linux
- *(manager)* set safenode service KillMode to fix restarts
- use TcpListener to check for free RPC port
- *(test)* update control mock test
- *(manager)* obtain node listen addr instead using the rpc
- *(manager)* increase port unbinding time

### Other
- *(release)* sn_protocol-v0.15.0
- get clippy to stop mentioning this
- *(daemon)* rename daemon binary to safenodemand
- *(manager)* add daemon restart test
- *(daemon)* add more context to errors
- *(manager)* removing support for process restarts
- create a `faucet_control` module
- *(release)* sn_cli-v0.89.80/sn_client-v0.104.27/sn_networking-v0.13.29/sn_transfers-v0.16.0/sn_faucet-v0.3.80/sn_metrics-v/sn_node-v0.104.35/sn-node-manager-v0.3.11/sn_node_rpc_client-v0.4.66/token_supplies-v0.1.43/sn_protocol-v0.14.8
- cleanup version in node_manager after experimentation
- *(release)* sn_cli-v0.89.79/sn_client-v0.104.26/sn_protocol-v0.14.7/sn_faucet-v0.3.79/sn_node-v0.104.34/sn-node-manager-v0.3.10/sn_node_rpc_client-v0.4.65/sn_networking-v0.13.28
- *(release)* sn_protocol-v0.14.6/sn_node-v0.104.33/sn-node-manager-v0.3.9/sn_cli-v0.89.78/sn_client-v0.104.25/sn_networking-v0.13.27/sn_node_rpc_client-v0.4.64
- *(deps)* update service manager to the latest version
- *(manager)* move node controls into its own module
- *(manager)* make ServiceControl more generic
- *(manager)* remove panics from the codebase and instead propagate errors
- *(manager)* rename options to be coherent across the lib
- remove unused install file
- *(release)* sn-node-manager-v0.3.8
- *(release)* sn_cli-v0.89.77/sn_client-v0.104.24/sn_faucet-v0.3.76/sn_node-v0.104.32/sn_node_rpc_client-v0.4.63
- *(release)* sn_cli-v0.89.76/sn_faucet-v0.3.75/sn_node_rpc_client-v0.4.62/sn-node-manager-v0.3.7
- *(release)* sn_networking-v0.13.26/sn-node-manager-v0.3.6/sn_client-v0.104.23/sn_node-v0.104.31
- *(release)* sn_cli-v0.89.75/sn_client-v0.104.22/sn_networking-v0.13.25/sn_transfers-v0.15.8/sn_protocol-v0.14.5/sn_faucet-v0.3.74/sn_node-v0.104.30/sn_node_rpc_client-v0.4.61
- *(release)* sn_cli-v0.89.74/sn_client-v0.104.21/sn_networking-v0.13.24/sn_transfers-v0.15.7/sn-node-manager-v0.3.5/sn_protocol-v0.14.4/sn_faucet-v0.3.73/sn_node-v0.104.29/sn_node_rpc_client-v0.4.60
- *(release)* sn_client-v0.104.20/sn_registers-v0.3.10/sn_node-v0.104.28/sn_cli-v0.89.73/sn_protocol-v0.14.3/sn_faucet-v0.3.72/sn_node_rpc_client-v0.4.59
- *(release)* sn_cli-v0.89.72/sn_client-v0.104.19/sn_faucet-v0.3.71/sn-node-manager-v0.3.4/sn_node-v0.104.27/sn_node_rpc_client-v0.4.58
- *(release)* sn_networking-v0.13.23/sn_node-v0.104.26/sn_client-v0.104.18/sn_node_rpc_client-v0.4.57
- *(release)* sn_cli-v0.89.71/sn_networking-v0.13.22/sn_faucet-v0.3.70/sn_node_rpc_client-v0.4.56/sn-node-manager-v0.3.3/sn_client-v0.104.17/sn_node-v0.104.25
- *(release)* sn_cli-v0.89.69/sn_client-v0.104.15/sn_transfers-v0.15.6/sn_node-v0.104.23/sn_node_rpc_client-v0.4.55/sn-node-manager-v0.3.2/sn_networking-v0.13.20/sn_protocol-v0.14.2/sn_faucet-v0.3.68
- *(release)* sn_protocol-v0.14.1/sn-node-manager-v0.3.1/sn_cli-v0.89.68/sn_client-v0.104.13/sn_networking-v0.13.18/sn_node-v0.104.21/sn_node_rpc_client-v0.4.54
- *(release)* sn_protocol-v0.14.0/sn-node-manager-v0.3.0/sn_cli-v0.89.67/sn_client-v0.104.12/sn_networking-v0.13.17/sn_node-v0.104.20/sn_node_rpc_client-v0.4.53
- *(docs)* update based on comments
- *(release)* sn_cli-v0.89.66/sn_client-v0.104.11/sn_protocol-v0.13.1/sn_transfers-v0.15.5/sn_faucet-v0.3.65/sn_networking-v0.13.16/sn_node-v0.104.19/sn_node_rpc_client-v0.4.52/sn-node-manager-v0.2.1
- *(release)* sn_protocol-v0.13.0/sn-node-manager-v0.2.0/sn_cli-v0.89.65/sn_client-v0.104.10/sn_networking-v0.13.15/sn_node-v0.104.18/sn_node_rpc_client-v0.4.51
- *(manager)* move bootstrap_peers store step inside add fn
- *(protocol)* [**breaking**] make node dirs not optional
- *(release)* sn_node-v0.104.17/sn-node-manager-v0.1.62/sn_node_rpc_client-v0.4.50
- *(release)* sn_cli-v0.89.64/sn_client-v0.104.9/sn_transfers-v0.15.4/sn_networking-v0.13.14/sn_protocol-v0.12.7/sn_faucet-v0.3.64/sn_node-v0.104.16/sn_node_rpc_client-v0.4.49
- *(release)* sn-node-manager-v0.1.61
- *(release)* sn_node-v0.104.15/sn_node_rpc_client-v0.4.48
- *(release)* sn_node-v0.104.14/sn_node_rpc_client-v0.4.47/sn-node-manager-v0.1.60
- *(release)* sn_networking-v0.13.12/sn_node-v0.104.12/sn-node-manager-v0.1.59/sn_client-v0.104.7/sn_node_rpc_client-v0.4.46
- *(release)* sn_cli-v0.89.62/sn_client-v0.104.6/sn_node-v0.104.11/sn_faucet-v0.3.62/sn_node_rpc_client-v0.4.45
- *(release)* sn_cli-v0.89.61/sn_faucet-v0.3.60/sn_node-v0.104.10/sn_node_rpc_client-v0.4.44/sn-node-manager-v0.1.58
- *(release)* sn_cli-v0.89.60/sn_networking-v0.13.11/sn_faucet-v0.3.59/sn_node_rpc_client-v0.4.43/sn_client-v0.104.5/sn_node-v0.104.9/sn-node-manager-v0.1.57
- *(release)* sn_cli-v0.89.59/sn_faucet-v0.3.58/sn_node-v0.104.7/sn_node_rpc_client-v0.4.42
- *(release)* sn_build_info-v0.1.5/sn_cli-v0.89.58/sn_client-v0.104.3/sn_networking-v0.13.9/sn_protocol-v0.12.6/sn_registers-v0.3.9/sn_transfers-v0.15.3/sn_peers_acquisition-v0.2.6/sn_logging-v0.2.21/sn_faucet-v0.3.57/sn_node-v0.104.6/sn_node_rpc_client-v0.4.41/sn-node-manager-v0.1.56/token_supplies-v0.1.41
- obtain the version number from binary
- *(release)* sn_cli-v0.89.56/sn_client-v0.104.2/sn_networking-v0.13.8/sn_protocol-v0.12.5/sn_faucet-v0.3.55/sn_node-v0.104.5/sn_node_rpc_client-v0.4.40/sn-node-manager-v0.1.55/token_supplies-v0.1.40
- *(release)* sn_cli-v0.89.55/sn_networking-v0.13.7/sn_faucet-v0.3.54/sn_node-v0.104.4/sn_node_rpc_client-v0.4.39/sn-node-manager-v0.1.54/token_supplies-v0.1.39/sn_client-v0.104.1
- *(release)* sn_cli-v0.89.54/sn_faucet-v0.3.53/sn_node-v0.104.3/sn_node_rpc_client-v0.4.38/sn-node-manager-v0.1.53/token_supplies-v0.1.38
- *(release)* sn_cli-v0.89.53/sn_logging-v0.2.20/sn_faucet-v0.3.52/sn_node-v0.104.2/sn_node_rpc_client-v0.4.37/sn-node-manager-v0.1.52/token_supplies-v0.1.37
- *(release)* sn_cli-v0.89.52/sn_faucet-v0.3.51/sn_node-v0.104.1/sn_node_rpc_client-v0.4.36/sn-node-manager-v0.1.51/token_supplies-v0.1.36
- improvements from dev feedback
- *(release)* sn_cli-v0.89.51/sn_client-v0.104.0/sn_faucet-v0.3.50/sn_node-v0.104.0/sn_node_rpc_client-v0.4.35
- *(release)* sn_cli-v0.89.50/sn_client-v0.103.7/sn_transfers-v0.15.2/sn_faucet-v0.3.49/sn_node-v0.103.46/sn_node_rpc_client-v0.4.34/sn-node-manager-v0.1.50/token_supplies-v0.1.35/sn_networking-v0.13.6/sn_protocol-v0.12.4
- *(release)* sn_cli-v0.89.49/sn_transfers-v0.15.1/sn_faucet-v0.3.48/sn_node-v0.103.45/sn_node_rpc_client-v0.4.33/sn-node-manager-v0.1.49/token_supplies-v0.1.34/sn_client-v0.103.6/sn_networking-v0.13.5/sn_protocol-v0.12.3
- *(release)* sn_cli-v0.89.48/sn_faucet-v0.3.47/sn_node-v0.103.44/sn_node_rpc_client-v0.4.32/sn-node-manager-v0.1.48/token_supplies-v0.1.33
- *(release)* sn_cli-v0.89.47/sn_faucet-v0.3.46/sn_node-v0.103.43/sn_node_rpc_client-v0.4.31/sn-node-manager-v0.1.47/token_supplies-v0.1.32
- *(release)* sn_cli-v0.89.46/sn_networking-v0.13.4/sn_faucet-v0.3.45/sn_node-v0.103.42/sn_node_rpc_client-v0.4.30/sn-node-manager-v0.1.46/sn_client-v0.103.5
- *(release)* sn_cli-v0.89.45/sn_networking-v0.13.3/sn_faucet-v0.3.44/sn_node-v0.103.41/sn_node_rpc_client-v0.4.29/sn-node-manager-v0.1.45/token_supplies-v0.1.31/sn_client-v0.103.4
- *(release)* sn_cli-v0.89.44/sn_faucet-v0.3.43/sn_node-v0.103.40/sn_node_rpc_client-v0.4.28/sn-node-manager-v0.1.44/token_supplies-v0.1.30
- *(release)* sn_cli-v0.89.43/sn_faucet-v0.3.42/sn_node-v0.103.39/sn_node_rpc_client-v0.4.27/sn-node-manager-v0.1.43/token_supplies-v0.1.29
- *(release)* sn_cli-v0.89.42/sn_client-v0.103.3/sn_faucet-v0.3.41/sn_node-v0.103.38/sn_node_rpc_client-v0.4.26/sn-node-manager-v0.1.42
- *(release)* sn_cli-v0.89.41/sn_protocol-v0.12.2/sn_faucet-v0.3.40/sn_node-v0.103.37/sn_node_rpc_client-v0.4.25/sn-node-manager-v0.1.41/token_supplies-v0.1.28/sn_client-v0.103.2/sn_networking-v0.13.2
- *(release)* sn_cli-v0.89.40/sn_faucet-v0.3.39/sn_node-v0.103.36/sn_node_rpc_client-v0.4.24/sn-node-manager-v0.1.40/token_supplies-v0.1.27
- *(release)* sn_cli-v0.89.39/sn_networking-v0.13.1/sn_faucet-v0.3.38/sn_node-v0.103.35/sn_node_rpc_client-v0.4.23/sn-node-manager-v0.1.39/token_supplies-v0.1.26/sn_client-v0.103.1
- *(release)* sn_client-v0.103.0/sn_networking-v0.13.0/sn_transfers-v0.15.0/sn_protocol-v0.12.1
- *(release)* sn_cli-v0.89.38/sn_faucet-v0.3.37/sn_node-v0.103.34/sn_node_rpc_client-v0.4.22/sn-node-manager-v0.1.38/token_supplies-v0.1.25
- *(release)* sn_cli-v0.89.37/sn_networking-v0.12.46/sn_faucet-v0.3.36/sn_node-v0.103.33/sn_node_rpc_client-v0.4.21/sn-node-manager-v0.1.37/token_supplies-v0.1.24/sn_client-v0.102.22
- *(release)* sn_cli-v0.89.36/sn_client-v0.102.21/sn_networking-v0.12.45/sn_faucet-v0.3.35/sn_node-v0.103.32/sn_node_rpc_client-v0.4.20/sn-node-manager-v0.1.36/token_supplies-v0.1.23
- *(release)* sn_cli-v0.89.35/sn_networking-v0.12.44/sn_faucet-v0.3.34/sn_node-v0.103.31/sn_node_rpc_client-v0.4.19/sn-node-manager-v0.1.35/token_supplies-v0.1.22/sn_client-v0.102.20
- *(release)* sn_cli-v0.89.34/sn_logging-v0.2.18/sn_faucet-v0.3.33/sn_node-v0.103.30/sn_node_rpc_client-v0.4.18/sn-node-manager-v0.1.34/token_supplies-v0.1.21
- download binary once for `add` command
- misc clean up for local testnets
- *(release)* sn_cli-v0.89.33/sn_client-v0.102.19/sn_faucet-v0.3.32/sn_node-v0.103.29/sn_node_rpc_client-v0.4.17/sn-node-manager-v0.1.33/sn_testnet-v0.3.50/token_supplies-v0.1.20
- *(release)* sn_networking-v0.12.43/sn_protocol-v0.12.0/sn_client-v0.102.18
- *(manager)* remove unused service method
- *(manager)* add back node_port option without port checking
- *(release)* sn_cli-v0.89.32/sn_faucet-v0.3.31/sn_node-v0.103.28/sn_node_rpc_client-v0.4.16/sn-node-manager-v0.1.32/sn_testnet-v0.3.49/token_supplies-v0.1.19
- *(release)* sn_cli-v0.89.31/sn_client-v0.102.17/sn_faucet-v0.3.30/sn_node-v0.103.27/sn_node_rpc_client-v0.4.15/sn-node-manager-v0.1.31/sn_testnet-v0.3.48/token_supplies-v0.1.18
- *(release)* sn_cli-v0.89.30/sn_faucet-v0.3.29/sn_node-v0.103.26/sn_node_rpc_client-v0.4.14/sn-node-manager-v0.1.30/sn_testnet-v0.3.47/token_supplies-v0.1.17
- *(release)* sn_cli-v0.89.29/sn_client-v0.102.16/sn_faucet-v0.3.28/sn_node-v0.103.25/sn_node_rpc_client-v0.4.13/sn-node-manager-v0.1.29/sn_testnet-v0.3.46/token_supplies-v0.1.16
- *(release)* sn_cli-v0.89.28/sn_networking-v0.12.42/sn_faucet-v0.3.27/sn_node-v0.103.24/sn_node_rpc_client-v0.4.12/sn-node-manager-v0.1.28/sn_testnet-v0.3.45/token_supplies-v0.1.15/sn_client-v0.102.15
- *(release)* sn_cli-v0.89.27/sn_protocol-v0.11.3/sn_faucet-v0.3.26/sn_node-v0.103.23/sn_node_rpc_client-v0.4.11/sn-node-manager-v0.1.27/sn_testnet-v0.3.44/token_supplies-v0.1.14/sn_client-v0.102.14/sn_networking-v0.12.41
- *(manager)* provide rpc address instead of rpc port
- *(release)* sn_cli-v0.89.26/sn_faucet-v0.3.25/sn_node-v0.103.22/sn_node_rpc_client-v0.4.10/sn-node-manager-v0.1.26/sn_testnet-v0.3.43/token_supplies-v0.1.13
- *(manager)* make VerbosityLevel a public type
- *(release)* sn_cli-v0.89.25/sn_node-v0.103.21/sn_node_rpc_client-v0.4.9/sn-node-manager-v0.1.25/sn_testnet-v0.3.42/token_supplies-v0.1.12
- provide verbosity level
- improve error handling for `start` command
- improve error handling for `add` command
- version and url arguments conflict
- *(release)* sn_cli-v0.89.23/sn_client-v0.102.13/sn_transfers-v0.14.43/sn_networking-v0.12.40/sn_protocol-v0.11.2
- *(release)* sn_cli-v0.89.22/sn_faucet-v0.3.22/sn_node-v0.103.20/sn_node_rpc_client-v0.4.8/sn-node-manager-v0.1.24/sn_testnet-v0.3.41/token_supplies-v0.1.11
- *(release)* sn_cli-v0.89.21/sn_faucet-v0.3.21/sn_node-v0.103.19/sn_node_rpc_client-v0.4.7/sn-node-manager-v0.1.23/sn_testnet-v0.3.40/token_supplies-v0.1.10
- *(release)* sn_cli-v0.89.20/sn_faucet-v0.3.20/sn_node-v0.103.18/sn_node_rpc_client-v0.4.6/sn-node-manager-v0.1.22/sn_testnet-v0.3.39/token_supplies-v0.1.9
- *(release)* sn_cli-v0.89.19/sn_client-v0.102.12/sn_faucet-v0.3.19/sn_node-v0.103.17/sn_node_rpc_client-v0.4.5/sn-node-manager-v0.1.21/sn_testnet-v0.3.38/token_supplies-v0.1.8
- *(release)* sn_cli-v0.89.18/sn_networking-v0.12.39/sn_faucet-v0.3.18/sn_node-v0.103.16/sn_node_rpc_client-v0.4.4/sn-node-manager-v0.1.20/sn_testnet-v0.3.37/token_supplies-v0.1.7/sn_client-v0.102.11
- rename sn_node_manager crate
- *(manager)* rename node manager crate

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
