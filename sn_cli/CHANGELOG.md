# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.77.16](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.15...sn_cli-v0.77.16) - 2023-06-14

### Other
- update dependencies

## [0.77.15](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.14...sn_cli-v0.77.15) - 2023-06-14

### Other
- use clap env and parse multiaddr

## [0.77.14](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.13...sn_cli-v0.77.14) - 2023-06-14

### Added
- *(client)* expose req/resp timeout to client cli

### Other
- *(client)* parse duration in clap derivation

## [0.77.13](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.12...sn_cli-v0.77.13) - 2023-06-13

### Other
- update dependencies

## [0.77.12](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.11...sn_cli-v0.77.12) - 2023-06-13

### Other
- update dependencies

## [0.77.11](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.10...sn_cli-v0.77.11) - 2023-06-12

### Other
- update dependencies

## [0.77.10](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.9...sn_cli-v0.77.10) - 2023-06-12

### Other
- update dependencies

## [0.77.9](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.8...sn_cli-v0.77.9) - 2023-06-09

### Other
- improve documentation for cli commands

## [0.77.8](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.7...sn_cli-v0.77.8) - 2023-06-09

### Other
- manually change crate version

## [0.77.7](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.6...sn_cli-v0.77.7) - 2023-06-09

### Other
- update dependencies

## [0.77.6](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.5...sn_cli-v0.77.6) - 2023-06-09

### Other
- emit git info with vergen

## [0.77.5](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.4...sn_cli-v0.77.5) - 2023-06-09

### Other
- update dependencies

## [0.77.4](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.3...sn_cli-v0.77.4) - 2023-06-09

### Other
- provide clarity on command arguments

## [0.77.3](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.2...sn_cli-v0.77.3) - 2023-06-08

### Other
- update dependencies

## [0.77.2](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.1...sn_cli-v0.77.2) - 2023-06-08

### Other
- improve documentation for cli arguments

## [0.77.1](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.0...sn_cli-v0.77.1) - 2023-06-07

### Added
- making the CLI --peer arg global so it can be passed in any order
- bail out if empty list of addreses is provided for payment proof generation
- *(client)* add progress indicator for initial network connections
- attach payment proof when uploading Chunks
- collect payment proofs and make sure merkletree always has pow-of-2 leaves
- node side payment proof validation from a given Chunk, audit trail, and reason-hash
- use all Chunks of a file to generate payment the payment proof tree
- Chunk storage payment and building payment proofs

### Other
- Revert "chore(release): sn_cli-v0.77.1/sn_client-v0.85.2/sn_networking-v0.1.2/sn_node-v0.83.1"
- improve CLI --peer arg doc
- *(release)* sn_cli-v0.77.1/sn_client-v0.85.2/sn_networking-v0.1.2/sn_node-v0.83.1
- Revert "chore(release): sn_cli-v0.77.1/sn_client-v0.85.2/sn_networking-v0.1.2/sn_protocol-v0.1.2/sn_node-v0.83.1/sn_record_store-v0.1.2/sn_registers-v0.1.2"
- *(release)* sn_cli-v0.77.1/sn_client-v0.85.2/sn_networking-v0.1.2/sn_protocol-v0.1.2/sn_node-v0.83.1/sn_record_store-v0.1.2/sn_registers-v0.1.2
- *(logs)* enable metrics feature by default
- small log wording updates
- making Chunk payment proof optional for now
- moving all payment proofs utilities into sn_transfers crate
