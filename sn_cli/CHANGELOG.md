# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
