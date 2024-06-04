# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.3](https://github.com/joshuef/safe_network/compare/sn_peers_acquisition-v0.3.2...sn_peers_acquisition-v0.3.3) - 2024-06-04

### Other
- updated the following local packages: sn_networking

## [0.3.2](https://github.com/joshuef/safe_network/compare/sn_peers_acquisition-v0.3.1...sn_peers_acquisition-v0.3.2) - 2024-06-04

### Other
- *(release)* sn_client-v0.107.3/sn_transfers-v0.18.4/sn_cli-v0.93.2/sn_node-v0.107.2/node-launchpad-v0.3.2/sn-node-manager-v0.9.2/sn_auditor-v0.1.20/sn_networking-v0.16.2/sn_protocol-v0.17.2/sn_faucet-v0.4.22/sn_service_management-v0.3.3/sn_node_rpc_client-v0.6.20

## [0.3.1](https://github.com/joshuef/safe_network/compare/sn_peers_acquisition-v0.3.0...sn_peers_acquisition-v0.3.1) - 2024-06-03

### Other
- updated the following local packages: sn_networking

## [0.3.0](https://github.com/joshuef/safe_network/compare/sn_peers_acquisition-v0.2.12...sn_peers_acquisition-v0.3.0) - 2024-06-03

### Added
- *(launchpad)* use nat detection server to determine the nat status
- *(network)* [**breaking**] move network versioning away from sn_protocol

### Other
- *(release)* sn_auditor-v0.1.17/sn_client-v0.106.3/sn_networking-v0.15.3/sn_transfers-v0.18.1/sn_logging-v0.2.27/sn_cli-v0.92.0/sn_faucet-v0.4.19/sn_node-v0.106.5/sn_service_management-v0.3.0/node-launchpad-v0.2.0/sn-node-manager-v0.8.0/sn_protocol-v0.16.7/sn_node_rpc_client-v0.6.18

## [0.2.12](https://github.com/maidsafe/safe_network/compare/sn_peers_acquisition-v0.2.11...sn_peers_acquisition-v0.2.12) - 2024-05-08

### Other
- updated the following local packages: sn_protocol

## [0.2.11-alpha.1](https://github.com/maidsafe/safe_network/compare/sn_peers_acquisition-v0.2.11-alpha.0...sn_peers_acquisition-v0.2.11-alpha.1) - 2024-05-07

### Added
- *(tui)* adding services
- *(network)* network contacts url should point to the correct network version

### Fixed
- *(manager)* do not print to stdout on low verbosity level
- *(protocol)* evaluate NETWORK_VERSION_MODE at compile time

### Other
- *(versions)* sync versions with latest crates.io vs
- use quic again
- remove quic
- *(release)* sn_auditor-v0.1.7/sn_client-v0.105.3/sn_networking-v0.14.4/sn_protocol-v0.16.3/sn_build_info-v0.1.7/sn_transfers-v0.17.2/sn_peers_acquisition-v0.2.10/sn_cli-v0.90.4/sn_faucet-v0.4.9/sn_metrics-v0.1.4/sn_node-v0.105.6/sn_service_management-v0.2.4/sn-node-manager-v0.7.4/sn_node_rpc_client-v0.6.8/token_supplies-v0.1.47
- *(release)* sn_auditor-v0.1.7/sn_client-v0.105.3/sn_networking-v0.14.4/sn_protocol-v0.16.3/sn_build_info-v0.1.7/sn_transfers-v0.17.2/sn_peers_acquisition-v0.2.10/sn_cli-v0.90.4/sn_faucet-v0.4.9/sn_metrics-v0.1.4/sn_node-v0.105.6/sn_service_management-v0.2.4/sn-node-manager-v0.7.4/sn_node_rpc_client-v0.6.8/token_supplies-v0.1.47
- *(release)* sn_client-v0.105.3-alpha.5/sn_protocol-v0.16.3-alpha.2/sn_cli-v0.90.4-alpha.5/sn_node-v0.105.6-alpha.4/sn-node-manager-v0.7.4-alpha.1/sn_auditor-v0.1.7-alpha.0/sn_networking-v0.14.4-alpha.0/sn_peers_acquisition-v0.2.10-alpha.0/sn_faucet-v0.4.9-alpha.0/sn_service_management-v0.2.4-alpha.0/sn_node_rpc_client-v0.6.8-alpha.0
- *(release)* sn_client-v0.105.3-alpha.3/sn_protocol-v0.16.3-alpha.1/sn_peers_acquisition-v0.2.9-alpha.2/sn_cli-v0.90.4-alpha.3/sn_node-v0.105.6-alpha.1/sn_auditor-v0.1.5-alpha.0/sn_networking-v0.14.3-alpha.0/sn_faucet-v0.4.7-alpha.0/sn_service_management-v0.2.3-alpha.0/sn-node-manager-v0.7.4-alpha.0/sn_node_rpc_client-v0.6.6-alpha.0
- *(release)* sn_auditor-v0.1.3-alpha.1/sn_client-v0.105.3-alpha.1/sn_networking-v0.14.2-alpha.1/sn_peers_acquisition-v0.2.9-alpha.1/sn_cli-v0.90.4-alpha.1/sn_metrics-v0.1.4-alpha.0/sn_node-v0.105.5-alpha.1/sn_service_management-v0.2.2-alpha.1/sn-node-manager-v0.7.3-alpha.1/sn_node_rpc_client-v0.6.4-alpha.1/token_supplies-v0.1.47-alpha.0
- *(release)* sn_build_info-v0.1.7-alpha.1/sn_protocol-v0.16.3-alpha.0/sn_cli-v0.90.4-alpha.0/sn_faucet-v0.4.5-alpha.0/sn_node-v0.105.5-alpha.0
- *(release)* sn_auditor-v0.1.3-alpha.0/sn_client-v0.105.3-alpha.0/sn_networking-v0.14.2-alpha.0/sn_protocol-v0.16.2-alpha.0/sn_build_info-v0.1.7-alpha.0/sn_transfers-v0.17.2-alpha.0/sn_peers_acquisition-v0.2.9-alpha.0/sn_cli-v0.90.3-alpha.0/sn_node-v0.105.4-alpha.0/sn-node-manager-v0.7.3-alpha.0/sn_faucet-v0.4.4-alpha.0/sn_service_management-v0.2.2-alpha.0/sn_node_rpc_client-v0.6.4-alpha.0

## [0.2.8](https://github.com/joshuef/safe_network/compare/sn_peers_acquisition-v0.2.7...sn_peers_acquisition-v0.2.8) - 2024-03-14

### Other
- fix logging logic

## [0.2.6](https://github.com/maidsafe/safe_network/compare/sn_peers_acquisition-v0.2.5...sn_peers_acquisition-v0.2.6) - 2024-02-08

### Other
- copyright update to current year

## [0.2.5](https://github.com/maidsafe/safe_network/compare/sn_peers_acquisition-v0.2.4...sn_peers_acquisition-v0.2.5) - 2024-01-25

### Added
- client webtransport-websys feat

## [0.2.4](https://github.com/maidsafe/safe_network/compare/sn_peers_acquisition-v0.2.3...sn_peers_acquisition-v0.2.4) - 2024-01-24

### Added
- initial webtransport-websys wasm setup

### Other
- tidy up wasm32 as target arch rather than a feat

## [0.2.3](https://github.com/maidsafe/safe_network/compare/sn_peers_acquisition-v0.2.2...sn_peers_acquisition-v0.2.3) - 2024-01-18

### Added
- set quic as default transport

## [0.2.2](https://github.com/maidsafe/safe_network/compare/sn_peers_acquisition-v0.2.1...sn_peers_acquisition-v0.2.2) - 2024-01-16

### Other
- remove arg and env variable combination

## [0.2.1](https://github.com/maidsafe/safe_network/compare/sn_peers_acquisition-v0.2.0...sn_peers_acquisition-v0.2.1) - 2024-01-11

### Other
- make `first` argument public

## [0.2.0](https://github.com/maidsafe/safe_network/compare/sn_peers_acquisition-v0.1.14...sn_peers_acquisition-v0.2.0) - 2024-01-08

### Added
- provide `--first` argument for `safenode`

## [0.1.14](https://github.com/maidsafe/safe_network/compare/sn_peers_acquisition-v0.1.13...sn_peers_acquisition-v0.1.14) - 2024-01-08

### Other
- more doc updates to readme files

## [0.1.13](https://github.com/maidsafe/safe_network/compare/sn_peers_acquisition-v0.1.12...sn_peers_acquisition-v0.1.13) - 2023-12-08

### Fixed
- add missing clippy allow

## [0.1.12](https://github.com/maidsafe/safe_network/compare/sn_peers_acquisition-v0.1.11...sn_peers_acquisition-v0.1.12) - 2023-12-06

### Other
- add boilerplate for workspace lints

## [0.1.11](https://github.com/maidsafe/safe_network/compare/sn_peers_acquisition-v0.1.10...sn_peers_acquisition-v0.1.11) - 2023-12-01

### Other
- *(ci)* fix CI build cache parsing error

## [0.1.10](https://github.com/maidsafe/safe_network/compare/sn_peers_acquisition-v0.1.9...sn_peers_acquisition-v0.1.10) - 2023-11-22

### Added
- *(peers_acq)* shuffle peers before we return.

## [0.1.9](https://github.com/maidsafe/safe_network/compare/sn_peers_acquisition-v0.1.8...sn_peers_acquisition-v0.1.9) - 2023-11-06

### Added
- *(deps)* upgrade libp2p to 0.53

## [0.1.8](https://github.com/maidsafe/safe_network/compare/sn_peers_acquisition-v0.1.7...sn_peers_acquisition-v0.1.8) - 2023-10-26

### Fixed
- always put SAFE_PEERS as one of the bootstrap peer, if presents

## [0.1.7](https://github.com/maidsafe/safe_network/compare/sn_peers_acquisition-v0.1.6...sn_peers_acquisition-v0.1.7) - 2023-09-25

### Added
- *(peers)* use rustls-tls and readd https to the network-contacts url
- *(peers)* use a common way to bootstrap into the network for all the bins

### Fixed
- *(peers_acquisition)* bail on fail to parse peer id

### Other
- more logs around parsing network-contacts
- log the actual contacts url in messages

## [0.1.6](https://github.com/maidsafe/safe_network/compare/sn_peers_acquisition-v0.1.5...sn_peers_acquisition-v0.1.6) - 2023-08-30

### Other
- *(docs)* adjust --peer docs

## [0.1.5](https://github.com/maidsafe/safe_network/compare/sn_peers_acquisition-v0.1.4...sn_peers_acquisition-v0.1.5) - 2023-08-29

### Added
- *(node)* add feature flag for tcp/quic

## [0.1.4](https://github.com/maidsafe/safe_network/compare/sn_peers_acquisition-v0.1.3...sn_peers_acquisition-v0.1.4) - 2023-07-17

### Added
- *(networking)* upgrade to libp2p 0.52.0

## [0.1.3](https://github.com/maidsafe/safe_network/compare/sn_peers_acquisition-v0.1.2...sn_peers_acquisition-v0.1.3) - 2023-07-03

### Other
- various tidy up

## [0.1.2](https://github.com/maidsafe/safe_network/compare/sn_peers_acquisition-v0.1.1...sn_peers_acquisition-v0.1.2) - 2023-06-28

### Added
- *(node)* dial without PeerId

## [0.1.1](https://github.com/maidsafe/safe_network/compare/sn_peers_acquisition-v0.1.0...sn_peers_acquisition-v0.1.1) - 2023-06-14

### Other
- use clap env and parse multiaddr

## [0.1.0](https://github.com/jacderida/safe_network/releases/tag/sn_peers_acquisition-v0.1.0) - 2023-06-04

### Fixed
- *(node)* correct dead peer detection
- local-discovery deps
