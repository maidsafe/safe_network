# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.77.1](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.0...sn_cli-v0.77.1) - 2023-06-07

### Added
- *(client)* add progress indicator for initial network connections
- attach payment proof when uploading Chunks
- collect payment proofs and make sure merkletree always has pow-of-2 leaves
- node side payment proof validation from a given Chunk, audit trail, and reason-hash
- use all Chunks of a file to generate payment the payment proof tree
- Chunk storage payment and building payment proofs

### Other
- *(logs)* enable metrics feature by default
- small log wording updates
- making Chunk payment proof optional for now
- moving all payment proofs utilities into sn_transfers crate
