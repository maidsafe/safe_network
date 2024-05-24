# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.27](https://github.com/joshuef/safe_network/compare/sn_logging-v0.2.26...sn_logging-v0.2.27) - 2024-05-24

### Added
- *(nodeman)* add LogFormat as a startup arg for nodes

## [0.2.26-alpha.1](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.26-alpha.0...sn_logging-v0.2.26-alpha.1) - 2024-05-07

### Added
- make logging simpler to use
- *(log)* set log levels on the fly
- *(log)* use LogBuilder to initialize logging
- *(logging)* Add in SN_LOG=v for reduced networking logging
- [**breaking**] introduce `--log-format` arguments
- provide `--log-output-dest` arg for `safe`
- [**breaking**] provide `--log-output-dest` arg for `safenode`
- carry out validation for record_store::put
- provide option for log output in json
- *(node)* log PID of node w/ metrics in debug
- *(logging)* log metrics for safe and safenode bin
- add registers and transfers crates, deprecate domain
- *(logs)* add 'all' log shorthand
- add build_info crate

### Fixed
- do not create wallet on registry refresh
- logging, adapt program name
- *(logs)* enable faucet logs
- typos
- *(log)* capture logs from multiple integration tests
- *(log)* capture logs from tests
- *(logging)* get log name per bin
- add missing safenode/safe trace to  logs
- local-discovery deps
- remove unused deps, fix doc comment

### Other
- *(versions)* sync versions with latest crates.io vs
- *(deps)* bump dependencies
- *(release)* sn_auditor-v/sn_client-v0.105.0/sn_networking-v0.14.0/sn_metrics-v0.1.3/sn_protocol-v0.16.0/sn_registers-v0.3.12/sn_transfers-v0.17.0/sn_logging-v0.2.25/sn_cli-v0.90.0/sn_faucet-v0.4.0/sn_node-v0.105.0/sn_service_management-v0.2.0/sn-node-manager-v0.7.0/sn_node_rpc_client-v0.6.0/token_supplies-v0.1.46
- fix typo
- adapt client name for safe cli cmd
- *(release)* sn_cli-v0.89.85/sn_client-v0.104.31/sn_networking-v0.13.35/sn_protocol-v0.15.5/sn_transfers-v0.16.5/sn_logging-v0.2.24/sn_faucet-v0.3.85/sn_node-v0.104.41/sn_service_management-v0.1.2/sn-node-manager-v0.6.1/sn_node_rpc_client-v0.5.1/token_supplies-v0.1.45
- *(log)* add test to verify log reload functionality
- *(release)* sn_cli-v0.89.83/sn_client-v0.104.29/sn_networking-v0.13.33/sn_protocol-v0.15.4/sn_transfers-v0.16.4/sn_peers_acquisition-v0.2.8/sn_logging-v0.2.23/sn_faucet-v0.3.84/sn_node-v0.104.39/sn_service_management-v/sn-node-manager-v0.6.0/sn_node_rpc_client-v0.5.0/token_supplies-v0.1.44
- *(api)* make logging::Error public
- *(release)* initial alpha test release
- *(release)* sn_build_info-v0.1.5/sn_cli-v0.89.58/sn_client-v0.104.3/sn_networking-v0.13.9/sn_protocol-v0.12.6/sn_registers-v0.3.9/sn_transfers-v0.15.3/sn_peers_acquisition-v0.2.6/sn_logging-v0.2.21/sn_faucet-v0.3.57/sn_node-v0.104.6/sn_node_rpc_client-v0.4.41/sn-node-manager-v0.1.56/token_supplies-v0.1.41
- copyright update to current year
- *(release)* sn_cli-v0.89.53/sn_logging-v0.2.20/sn_faucet-v0.3.52/sn_node-v0.104.2/sn_node_rpc_client-v0.4.37/sn-node-manager-v0.1.52/token_supplies-v0.1.37
- Revert "chore: roll back to log more"
- *(release)* sn_logging-v0.2.19
- roll back to log more
- *(release)* sn_cli-v0.89.34/sn_logging-v0.2.18/sn_faucet-v0.3.33/sn_node-v0.103.30/sn_node_rpc_client-v0.4.18/sn-node-manager-v0.1.34/token_supplies-v0.1.21
- remove the `sn_testnet` crate
- *(release)* sn_cli-v0.89.11/sn_logging-v0.2.17/sn_faucet-v0.3.11/sn_node-v0.103.11/sn_node_rpc_client-v0.3.11/sn_testnet-v0.3.32/token_supplies-v0.1.2
- *(node)* reduce MAX_UNCOMPRESSED_LOG_FILES to 10
- *(release)* sn_build_info-v0.1.3/sn_cli-v0.86.43/sn_client-v0.99.6/sn_networking-v0.11.5/sn_protocol-v0.8.38/sn_registers-v0.3.5/sn_transfers-v0.14.26/sn_logging-v0.2.16/sn_peers_acquisition-v0.1.12/sn_faucet-v0.1.65/sn_node-v0.99.8/sn_node_rpc_client-v0.1.62/sn_testnet-v0.2.324
- remove needless pass by value
- use inline format args
- add boilerplate for workspace lints
- address failing clippy::all lints
- *(release)* sn_logging-v0.2.15
- *(release)* sn_cli-v0.84.22/sn_networking-v0.9.6/sn_registers-v0.3.3/sn_transfers-v0.14.7/sn_logging-v0.2.14/sn_node-v0.96.8/sn_testnet-v0.2.237/sn_client-v0.95.7/sn_protocol-v0.8.5
- *(release)* sn_cli-v0.84.15/sn_client-v0.95.1/sn_networking-v0.9.1/sn_logging-v0.2.13/sn_node-v0.96.1/sn_testnet-v0.2.230
- *(release)* sn_cli-v0.84.10/sn_client-v0.94.7/sn_protocol-v0.7.28/sn_logging-v0.2.12/sn_node-v0.95.5/sn_testnet-v0.2.225/sn_networking-v0.8.41
- more custom debug and debug skips
- *(release)* sn_cli-v0.83.39/sn_logging-v0.2.11/sn_node-v0.92.9/sn_testnet-v0.2.201
- *(release)* sn_cli-v0.83.14/sn_logging-v0.2.10/sn_node-v0.91.13/sn_testnet-v0.2.176
- *(logging)* reduce metric frequency and logged stats.
- *(release)* sn_cli-v0.81.54/sn_client-v0.89.20/sn_networking-v0.6.13/sn_transfers-v0.11.15/sn_logging-v0.2.9/sn_node-v0.90.24/sn_testnet-v0.2.145
- major dep updates
- *(release)* sn_cli-v0.81.40/sn_networking-v0.6.6/sn_transfers-v0.11.13/sn_logging-v0.2.8/sn_node-v0.90.10/sn_testnet-v0.2.131/sn_client-v0.89.10
- *(release)* sn_cli-v0.81.36/sn_client-v0.89.6/sn_networking-v0.6.5/sn_protocol-v0.6.9/sn_logging-v0.2.7/sn_node-v0.90.6/sn_testnet-v0.2.127/sn_transfers-v0.11.12
- remove unused error variants
- *(release)* sn_cli-v0.81.23/sn_logging-v0.2.6/sn_node-v0.89.23/sn_testnet-v0.2.114
- rotate logs after exceeding 20mb
- *(release)* sn_cli-v0.81.0/sn_client-v0.88.0/sn_networking-v0.5.0/sn_protocol-v0.6.0/sn_transfers-v0.11.0/sn_logging-v0.2.5/sn_node-v0.89.0/sn_testnet-v0.2.92
- *(deps)* bump tokio to 1.32.0
- *(release)* sn_cli-v0.80.49/sn_client-v0.87.18/sn_networking-v0.4.20/sn_logging-v0.2.4/sn_node-v0.88.38/sn_testnet-v0.2.76
- *(release)* sn_cli-v0.79.31/sn_client-v0.85.55/sn_networking-v0.3.27/sn_protocol-v0.2.10/sn_logging-v0.2.3/sn_node-v0.86.30/sn_testnet-v0.2.25/sn_transfers-v0.10.14
- cleanup error types
- *(release)* sn_cli-v0.79.18/sn_logging-v0.2.2/sn_node-v0.86.17/sn_testnet-v0.2.12
- *(clippy)* fix clippy warnings
- *(release)* sn_cli-v0.79.17/sn_logging-v0.2.1/sn_node-v0.86.16/sn_testnet-v0.2.11
- *(metrics)* remove network stats
- *(release)* sn_cli-v0.79.0/sn_logging-v0.2.0/sn_node-v0.86.0/sn_testnet-v0.1.76/sn_networking-v0.3.11
- *(release)* sn_cli-v0.78.24/sn_client-v0.85.38/sn_networking-v0.3.10/sn_logging-v0.1.5/sn_protocol-v0.2.1/sn_node-v0.85.9/sn_testnet-v0.1.74/sn_transfers-v0.10.4
- *(release)* sn_cli-v0.78.9/sn_logging-v0.1.4/sn_node-v0.83.55/sn_testnet-v0.1.59/sn_networking-v0.1.24
- *(logging)* dont log PID with metrics
- *(release)* sn_cli-v0.77.46/sn_logging-v0.1.3/sn_node-v0.83.42/sn_testnet-v0.1.46/sn_networking-v0.1.15
- *(release)* sn_cli-v0.77.12/sn_logging-v0.1.2/sn_node-v0.83.10/sn_testnet-v0.1.14/sn_networking-v0.1.6
- *(release)* sn_build_info-v0.1.1/sn_client-v0.85.1/sn_networking-v0.1.1/sn_logging-v0.1.1/sn_protocol-v0.1.1/sn_record_store-v0.1.1/sn_registers-v0.1.1
- admin for new crate publishing
- initial changelogs for new crates
- accommodate new workspace
- extract logging and networking crates

## [0.2.25](https://github.com/joshuef/safe_network/compare/sn_logging-v0.2.24...sn_logging-v0.2.25) - 2024-03-27

### Added
- make logging simpler to use

### Fixed
- logging, adapt program name

### Other
- fix typo
- adapt client name for safe cli cmd

## [0.2.24](https://github.com/joshuef/safe_network/compare/sn_logging-v0.2.23...sn_logging-v0.2.24) - 2024-03-21

### Added
- *(log)* set log levels on the fly

### Other
- *(log)* add test to verify log reload functionality

## [0.2.23](https://github.com/joshuef/safe_network/compare/sn_logging-v0.2.22...sn_logging-v0.2.23) - 2024-03-14

### Other
- *(api)* make logging::Error public

## [0.2.21](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.20...sn_logging-v0.2.21) - 2024-02-08

### Other
- copyright update to current year

## [0.2.20](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.19...sn_logging-v0.2.20) - 2024-02-08

### Other
- Revert "chore: roll back to log more"

## [0.2.19](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.18...sn_logging-v0.2.19) - 2024-02-06

### Other
- roll back to log more

## [0.2.18](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.17...sn_logging-v0.2.18) - 2024-01-31

### Other
- remove the `sn_testnet` crate

## [0.2.17](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.16...sn_logging-v0.2.17) - 2024-01-23

### Other
- *(node)* reduce MAX_UNCOMPRESSED_LOG_FILES to 10

## [0.2.16](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.15...sn_logging-v0.2.16) - 2023-12-06

### Other
- remove needless pass by value
- use inline format args
- add boilerplate for workspace lints
- address failing clippy::all lints

## [0.2.15](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.14...sn_logging-v0.2.15) - 2023-11-21

### Fixed
- *(logs)* enable faucet logs

## [0.2.14](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.13...sn_logging-v0.2.14) - 2023-10-26

### Fixed
- typos

## [0.2.13](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.12...sn_logging-v0.2.13) - 2023-10-24

### Added
- *(log)* use LogBuilder to initialize logging

## [0.2.12](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.11...sn_logging-v0.2.12) - 2023-10-23

### Other
- more custom debug and debug skips

## [0.2.11](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.10...sn_logging-v0.2.11) - 2023-10-11

### Fixed
- *(log)* capture logs from multiple integration tests
- *(log)* capture logs from tests

## [0.2.10](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.9...sn_logging-v0.2.10) - 2023-10-03

### Other
- *(logging)* reduce metric frequency and logged stats.

## [0.2.9](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.8...sn_logging-v0.2.9) - 2023-09-20

### Other
- major dep updates

## [0.2.8](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.7...sn_logging-v0.2.8) - 2023-09-15

### Added
- *(logging)* Add in SN_LOG=v for reduced networking logging

## [0.2.7](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.6...sn_logging-v0.2.7) - 2023-09-14

### Other
- remove unused error variants

## [0.2.6](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.5...sn_logging-v0.2.6) - 2023-09-06

### Other
- rotate logs after exceeding 20mb

## [0.2.5](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.4...sn_logging-v0.2.5) - 2023-08-30

### Other
- *(deps)* bump tokio to 1.32.0

## [0.2.4](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.3...sn_logging-v0.2.4) - 2023-08-17

### Fixed
- *(logging)* get log name per bin

## [0.2.3](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.2...sn_logging-v0.2.3) - 2023-07-20

### Other
- cleanup error types

## [0.2.2](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.1...sn_logging-v0.2.2) - 2023-07-13

### Other
- *(clippy)* fix clippy warnings

## [0.2.1](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.0...sn_logging-v0.2.1) - 2023-07-13

### Other
- *(metrics)* remove network stats

## [0.2.0](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.1.5...sn_logging-v0.2.0) - 2023-07-06

### Added
- introduce `--log-format` arguments
- provide `--log-output-dest` arg for `safe`
- provide `--log-output-dest` arg for `safenode`

## [0.1.5](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.1.4...sn_logging-v0.1.5) - 2023-07-05

### Added
- carry out validation for record_store::put

## [0.1.4](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.1.3...sn_logging-v0.1.4) - 2023-06-26

### Other
- *(logging)* dont log PID with metrics

## [0.1.3](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.1.2...sn_logging-v0.1.3) - 2023-06-21

### Added
- provide option for log output in json

## [0.1.2](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.1.1...sn_logging-v0.1.2) - 2023-06-13

### Added
- *(node)* log PID of node w/ metrics in debug

## [0.1.1](https://github.com/jacderida/safe_network/compare/sn_logging-v0.1.0...sn_logging-v0.1.1) - 2023-06-06

### Added
- *(logging)* log metrics for safe and safenode bin

## [0.1.0](https://github.com/jacderida/safe_network/releases/tag/sn_logging-v0.1.0) - 2023-06-04

### Added
- add registers and transfers crates, deprecate domain
- *(logs)* add 'all' log shorthand
- add build_info crate

### Fixed
- add missing safenode/safe trace to  logs
- local-discovery deps
- remove unused deps, fix doc comment

### Other
- accommodate new workspace
- extract logging and networking crates
