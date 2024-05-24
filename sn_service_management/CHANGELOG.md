# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0](https://github.com/joshuef/safe_network/compare/sn_service_management-v0.2.8...sn_service_management-v0.3.0) - 2024-05-24

### Added
- provide `--owner` arg for `add` cmd
- *(nodeman)* add LogFormat as a startup arg for nodes
- *(node_manager)* add auditor support
- provide `--upnp` flag for `add` command
- run safenode services in user mode
- [**breaking**] provide `--home-network` arg for `add` cmd
- distinguish failure to start during upgrade

### Fixed
- retain options on upgrade and prevent dup ports
- change reward balance to optional
- apply interval only to non-running nodes

### Other
- *(release)* sn_auditor-v0.1.16/sn_cli-v0.91.4/sn_faucet-v0.4.18/sn_metrics-v0.1.7/sn_node-v0.106.4/sn_service_management-v0.2.8/node-launchpad-v0.1.5/sn-node-manager-v0.7.7/sn_node_rpc_client-v0.6.17
- *(release)* sn_auditor-v0.1.15/sn_cli-v0.91.3/sn_faucet-v0.4.17/sn_metrics-v0.1.6/sn_node-v0.106.3/sn_service_management-v0.2.7/node-launchpad-v0.1.2/sn_node_rpc_client-v0.6.16
- upgrade service manager crate
- *(release)* sn_auditor-v0.1.13/sn_client-v0.106.1/sn_networking-v0.15.1/sn_protocol-v0.16.6/sn_cli-v0.91.1/sn_faucet-v0.4.15/sn_node-v0.106.1/node-launchpad-v0.1.1/sn_node_rpc_client-v0.6.14/sn_peers_acquisition-v0.2.12/sn_service_management-v0.2.6
- *(release)* sn_auditor-v0.1.12/sn_client-v0.106.0/sn_networking-v0.15.0/sn_transfers-v0.18.0/sn_peers_acquisition-v0.2.11/sn_logging-v0.2.26/sn_cli-v0.91.0/sn_faucet-v0.4.14/sn_metrics-v0.1.5/sn_node-v0.106.0/sn_service_management-v0.2.5/test_utils-v0.4.1/node-launchpad-v/sn-node-manager-v0.7.5/sn_node_rpc_client-v0.6.13/token_supplies-v0.1.48/sn_protocol-v0.16.5
- *(versions)* sync versions with latest crates.io vs
- use node registry for status
- [**breaking**] output reward balance in `status --json` cmd
- *(release)* sn_auditor-v0.1.7/sn_client-v0.105.3/sn_networking-v0.14.4/sn_protocol-v0.16.3/sn_build_info-v0.1.7/sn_transfers-v0.17.2/sn_peers_acquisition-v0.2.10/sn_cli-v0.90.4/sn_faucet-v0.4.9/sn_metrics-v0.1.4/sn_node-v0.105.6/sn_service_management-v0.2.4/sn-node-manager-v0.7.4/sn_node_rpc_client-v0.6.8/token_supplies-v0.1.47
- *(deps)* bump dependencies

## [0.2.8](https://github.com/maidsafe/safe_network/compare/sn_service_management-v0.2.7...sn_service_management-v0.2.8) - 2024-05-20

### Added
- *(node_manager)* add auditor support
- provide `--upnp` flag for `add` command

### Fixed
- retain options on upgrade and prevent dup ports

## [0.2.7](https://github.com/maidsafe/safe_network/compare/sn_service_management-v0.2.6...sn_service_management-v0.2.7) - 2024-05-15

### Added
- run safenode services in user mode

### Other
- upgrade service manager crate

## [0.2.6](https://github.com/maidsafe/safe_network/compare/sn_service_management-v0.2.5...sn_service_management-v0.2.6) - 2024-05-08

### Other
- updated the following local packages: sn_protocol

## [0.2.5-alpha.2](https://github.com/maidsafe/safe_network/compare/sn_service_management-v0.2.5-alpha.1...sn_service_management-v0.2.5-alpha.2) - 2024-05-07

### Added
- [**breaking**] provide `--home-network` arg for `add` cmd
- distinguish failure to start during upgrade

### Fixed
- change reward balance to optional
- apply interval only to non-running nodes

### Other
- *(versions)* sync versions with latest crates.io vs
- use node registry for status
- [**breaking**] output reward balance in `status --json` cmd
- clarify client::new description
- *(deps)* bump dependencies

## [0.2.1](https://github.com/joshuef/safe_network/compare/sn_service_management-v0.2.0...sn_service_management-v0.2.1) - 2024-03-28

### Other
- *(release)* sn_client-v0.105.1/sn_transfers-v0.17.1/sn_cli-v0.90.1/sn_faucet-v0.4.1/sn_node-v0.105.1/sn_auditor-v0.1.1/sn_networking-v0.14.1/sn_protocol-v0.16.1/sn-node-manager-v0.7.1/sn_node_rpc_client-v0.6.1

## [0.2.0](https://github.com/joshuef/safe_network/compare/sn_service_management-v0.1.2...sn_service_management-v0.2.0) - 2024-03-27

### Added
- [**breaking**] remove gossip code

### Fixed
- permit removal of manually removed services
- adding service user on alpine
- *(manager)* store exclusive reference to service data instead of cloning

## [0.1.2](https://github.com/joshuef/safe_network/compare/sn_service_management-v0.1.1...sn_service_management-v0.1.2) - 2024-03-21

### Added
- *(protocol)* add rpc to set node log level on the fly

## [0.1.1](https://github.com/joshuef/safe_network/compare/sn_service_management-v0.1.0...sn_service_management-v0.1.1) - 2024-03-18

### Fixed
- *(ci)* build packages separately to bypass feature unification process

## [0.1.0](https://github.com/joshuef/safe_network/releases/tag/sn_service_management-v0.1.0) - 2024-03-14

### Added
- add rpc to fetch status from the daemon

### Fixed
- *(manager)* don't error out when fetching pid for the daemon

### Other
- *(service)* remove the node service restart workaround
- extend `status` cmd for faucet and daemon
- correctly run node manager unit tests
- move rpc to its own module
- [**breaking**] uniform service management
- new `sn_service_management` crate
