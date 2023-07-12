# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.5](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.2.4...sn_protocol-v0.2.5) - 2023-07-12

### Other
- client to upload paid chunks in batches

## [0.2.4](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.2.3...sn_protocol-v0.2.4) - 2023-07-11

### Other
- logging detailed NetworkAddress

## [0.2.3](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.2.2...sn_protocol-v0.2.3) - 2023-07-10

### Added
- client query register via get_record
- client upload Register via put_record

## [0.2.2](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.2.1...sn_protocol-v0.2.2) - 2023-07-06

### Added
- client upload chunk using kad::put_record

## [0.2.1](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.2.0...sn_protocol-v0.2.1) - 2023-07-05

### Added
- carry out validation for record_store::put

## [0.2.0](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.1.11...sn_protocol-v0.2.0) - 2023-07-05

### Added
- [**breaking**] send the list of spent dbc ids instead of whole tx within payment proof
- check fee output id when spending inputs and check paid fee amount when storing Chunks

### Other
- adapting codebase to new sn_dbc

## [0.1.11](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.1.10...sn_protocol-v0.1.11) - 2023-07-04

### Other
- demystify permissions

## [0.1.10](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.1.9...sn_protocol-v0.1.10) - 2023-06-28

### Added
- rework permissions, implement register cmd handlers
- register refactor, kad reg without cmds

### Fixed
- rename UserRights to UserPermissions

## [0.1.9](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.1.8...sn_protocol-v0.1.9) - 2023-06-21

### Added
- *(node)* trigger replication when inactivity

## [0.1.8](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.1.7...sn_protocol-v0.1.8) - 2023-06-21

### Fixed
- *(protocol)* remove unsafe indexing

### Other
- remove unused error variants
- *(node)* obtain parent_tx from SignedSpend

## [0.1.7](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.1.6...sn_protocol-v0.1.7) - 2023-06-20

### Added
- *(network)* validate `Record` on GET
- *(network)* validate and store `ReplicatedData`
- *(node)* perform proper validations on PUT
- *(network)* store `Chunk` along with `PaymentProof`
- *(kad)* impl `RecordHeader` to store the record kind

### Fixed
- *(record_header)* encode unit enum as u32
- *(node)* store parent tx along with `SignedSpend`
- *(network)* use `rmp_serde` for `RecordHeader` ser/de

### Other
- *(docs)* add more docs and comments

## [0.1.6](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.1.5...sn_protocol-v0.1.6) - 2023-06-20

### Added
- nodes to verify input DBCs of Chunk payment proof were spent

### Other
- specific error types for different payment proof verification scenarios
- include the Tx instead of output DBCs as part of storage payment proofs

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
