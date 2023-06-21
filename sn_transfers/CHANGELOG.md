# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.9.7](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.9.6...sn_transfers-v0.9.7) - 2023-06-21

### Fixed
- *(sn_transfers)* hardcode new genesis DBC for tests

### Other
- *(node)* obtain parent_tx from SignedSpend

## [0.9.6](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.9.5...sn_transfers-v0.9.6) - 2023-06-20

### Other
- updated the following local packages: sn_protocol

## [0.9.5](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.9.4...sn_transfers-v0.9.5) - 2023-06-20

### Other
- specific error types for different payment proof verification scenarios

## [0.9.4](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.9.3...sn_transfers-v0.9.4) - 2023-06-15

### Added
- add double spend test

### Fixed
- parent spend checks
- parent spend issue

## [0.9.3](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.9.2...sn_transfers-v0.9.3) - 2023-06-14

### Added
- include output DBC within payment proof for Chunks storage

## [0.9.2](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.9.1...sn_transfers-v0.9.2) - 2023-06-12

### Added
- remove spendbook rw locks, improve logging

## [0.9.1](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.9.0...sn_transfers-v0.9.1) - 2023-06-09

### Other
- manually change crate version
