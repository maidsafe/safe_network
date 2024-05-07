# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
