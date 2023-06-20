# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.13](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.12...sn_networking-v0.1.13) - 2023-06-20

### Added
- *(sn_networking)* Make it possible to pass in a keypair for PeerID

## [0.1.12](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.11...sn_networking-v0.1.12) - 2023-06-20

### Other
- updated the following local packages: sn_protocol

## [0.1.11](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.10...sn_networking-v0.1.11) - 2023-06-20

### Other
- reduce some log levels to make 'debug' more useful

## [0.1.10](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.9...sn_networking-v0.1.10) - 2023-06-15

### Fixed
- parent spend checks
- parent spend issue

## [0.1.9](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.8...sn_networking-v0.1.9) - 2023-06-14

### Added
- include output DBC within payment proof for Chunks storage

## [0.1.8](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.7...sn_networking-v0.1.8) - 2023-06-14

### Added
- prune out of range record entries

## [0.1.7](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.6...sn_networking-v0.1.7) - 2023-06-14

### Added
- *(client)* increase default request timeout
- *(client)* expose req/resp timeout to client cli

### Other
- *(networking)* update naming of REQUEST_TIMEOUT_DEFAULT_S

## [0.1.6](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.5...sn_networking-v0.1.6) - 2023-06-13

### Other
- updated the following local packages: sn_logging

## [0.1.5](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.4...sn_networking-v0.1.5) - 2023-06-12

### Added
- remove spendbook rw locks, improve logging

## [0.1.4](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.3...sn_networking-v0.1.4) - 2023-06-12

### Other
- updated the following local packages: sn_record_store

## [0.1.3](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.2...sn_networking-v0.1.3) - 2023-06-09

### Other
- manually change crate version
- heavier load during the churning test
- *(client)* trival log improvement
- Revert "chore(release): sn_cli-v0.77.1/sn_client-v0.85.2/sn_networking-v0.1.2/sn_node-v0.83.1"

## [0.1.1](https://github.com/jacderida/safe_network/compare/sn_networking-v0.1.0...sn_networking-v0.1.1) - 2023-06-06

### Added
- refactor replication flow to using pull model
- *(node)* remove delay for Identify

### Other
- *(node)* return proper error if failing to create storage dir

## [0.1.0](https://github.com/jacderida/safe_network/releases/tag/sn_networking-v0.1.0) - 2023-06-04

### Added
- record based DBC Spends
- *(record_store)* extract record_store into its own crate

### Fixed
- expand channel capacity
- *(node)* correct dead peer detection
- *(node)* increase replication range to 5.
- add in init to potential_dead_peers.
- remove unused deps after crate reorg
- *(networking)* clippy
- local-discovery deps
- remove unused deps, fix doc comment

### Other
- increase networking channel size
- *(CI)* mem check against large file and churn test
- fixup after rebase
- extract logging and networking crates
