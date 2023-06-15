# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.5](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.1.4...sn_protocol-v0.1.5) - 2023-06-15

### Added
- add double spend test

### Fixed
- parent spend checks
- parent spend issue

## [0.1.4](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.1.3...sn_protocol-v0.1.4) - 2023-06-14

### Added
- include output DBC within payment proof for Chunks storage

## [0.1.3](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.1.2...sn_protocol-v0.1.3) - 2023-06-09

### Fixed
- *(replication)* prevent dropped conns during replication

### Other
- manually change crate version
- Revert "chore(release): sn_cli-v0.77.1/sn_client-v0.85.2/sn_networking-v0.1.2/sn_protocol-v0.1.2/sn_node-v0.83.1/sn_record_store-v0.1.2/sn_registers-v0.1.2"

## [0.1.1](https://github.com/jacderida/safe_network/compare/sn_protocol-v0.1.0...sn_protocol-v0.1.1) - 2023-06-06

### Added
- refactor replication flow to using pull model

## [0.1.0](https://github.com/jacderida/safe_network/releases/tag/sn_protocol-v0.1.0) - 2023-06-04

### Added
- store double spends when we detect them
- record based DBC Spends

### Fixed
- remove unused deps, fix doc comment

### Other
- bump sn_dbc version to 19 for simpler signedspend debug
- accommodate new workspace
- extract new sn_protocol crate
