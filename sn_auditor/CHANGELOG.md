# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.17](https://github.com/joshuef/safe_network/compare/sn_auditor-v0.1.16...sn_auditor-v0.1.17) - 2024-05-24

### Added
- *(auditor)* cache beta participants to the disk
- *(auditor)* add new beta participants via endpoint
- backup rewards json to disk regularly
- docs for sn_auditor
- offline mode for beta rewards
- upgrade cli audit to use DAG
- *(audit)* simplify reward output
- *(audit)* make svg processing a non-deafult feat
- *(audit)* accept line separated list of discord ids
- remove two uneeded env vars
- pass genesis_cn pub fields separate to hide sk
- pass sk_str via cli opt
- improve code to use existing utils
- tracking beta rewards from the DAG
- dag faults unit tests, sn_auditor offline mode

### Fixed
- *(auditor)* discord id cannot be empty
- *(auditor)* extend the beta particpants list
- auditor key arg to match docs
- dag and dag-svg feature mismatch
- beta rewards participants overwriting and renamings
- allow unknown discord IDs temporarily
- orphan parent bug, improve fault detection and logging

### Other
- move dag svg
- rename improperly named foundation_key
- *(release)* sn_auditor-v0.1.16/sn_cli-v0.91.4/sn_faucet-v0.4.18/sn_metrics-v0.1.7/sn_node-v0.106.4/sn_service_management-v0.2.8/node-launchpad-v0.1.5/sn-node-manager-v0.7.7/sn_node_rpc_client-v0.6.17
- *(release)* sn_auditor-v0.1.15/sn_cli-v0.91.3/sn_faucet-v0.4.17/sn_metrics-v0.1.6/sn_node-v0.106.3/sn_service_management-v0.2.7/node-launchpad-v0.1.2/sn_node_rpc_client-v0.6.16
- *(release)* sn_client-v0.106.2/sn_networking-v0.15.2/sn_cli-v0.91.2/sn_node-v0.106.2/sn_auditor-v0.1.14/sn_faucet-v0.4.16/sn_node_rpc_client-v0.6.15
- *(release)* sn_auditor-v0.1.13/sn_client-v0.106.1/sn_networking-v0.15.1/sn_protocol-v0.16.6/sn_cli-v0.91.1/sn_faucet-v0.4.15/sn_node-v0.106.1/node-launchpad-v0.1.1/sn_node_rpc_client-v0.6.14/sn_peers_acquisition-v0.2.12/sn_service_management-v0.2.6
- *(release)* sn_auditor-v0.1.12/sn_client-v0.106.0/sn_networking-v0.15.0/sn_transfers-v0.18.0/sn_peers_acquisition-v0.2.11/sn_logging-v0.2.26/sn_cli-v0.91.0/sn_faucet-v0.4.14/sn_metrics-v0.1.5/sn_node-v0.106.0/sn_service_management-v0.2.5/test_utils-v0.4.1/node-launchpad-v/sn-node-manager-v0.7.5/sn_node_rpc_client-v0.6.13/token_supplies-v0.1.48/sn_protocol-v0.16.5
- *(versions)* sync versions with latest crates.io vs
- *(release)* sn_auditor-v0.1.7/sn_client-v0.105.3/sn_networking-v0.14.4/sn_protocol-v0.16.3/sn_build_info-v0.1.7/sn_transfers-v0.17.2/sn_peers_acquisition-v0.2.10/sn_cli-v0.90.4/sn_faucet-v0.4.9/sn_metrics-v0.1.4/sn_node-v0.105.6/sn_service_management-v0.2.4/sn-node-manager-v0.7.4/sn_node_rpc_client-v0.6.8/token_supplies-v0.1.47
- *(deps)* bump dependencies

## [0.1.16](https://github.com/maidsafe/safe_network/compare/sn_auditor-v0.1.15...sn_auditor-v0.1.16) - 2024-05-20

### Other
- update Cargo.lock dependencies

## [0.1.15](https://github.com/maidsafe/safe_network/compare/sn_auditor-v0.1.14...sn_auditor-v0.1.15) - 2024-05-15

### Other
- update Cargo.lock dependencies

## [0.1.14](https://github.com/maidsafe/safe_network/compare/sn_auditor-v0.1.13...sn_auditor-v0.1.14) - 2024-05-09

### Other
- updated the following local packages: sn_client

## [0.1.13](https://github.com/maidsafe/safe_network/compare/sn_auditor-v0.1.12...sn_auditor-v0.1.13) - 2024-05-08

### Other
- update Cargo.lock dependencies

## [0.1.12-alpha.1](https://github.com/maidsafe/safe_network/compare/sn_auditor-v0.1.12-alpha.0...sn_auditor-v0.1.12-alpha.1) - 2024-05-07

### Other
- update Cargo.lock dependencies

## [0.1.2](https://github.com/maidsafe/safe_network/compare/sn_auditor-v0.1.1...sn_auditor-v0.1.2) - 2024-03-28

### Other
- updated the following local packages: sn_client

## [0.1.1](https://github.com/joshuef/safe_network/compare/sn_auditor-v0.1.0...sn_auditor-v0.1.1) - 2024-03-28

### Other
- updated the following local packages: sn_client

## [0.1.0](https://github.com/joshuef/safe_network/releases/tag/sn_auditor-v0.1.0) - 2024-03-27

### Added
- svg caching, fault tolerance during DAG collection
- make logging simpler to use
- introducing sn_auditor

### Fixed
- logging, adapt program name

### Other
- remove Cargo.lock
