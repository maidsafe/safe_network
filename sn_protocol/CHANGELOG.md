# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.1.1...sn_protocol-v0.1.2) - 2023-06-07

### Added
- attach payment proof when uploading Chunks

### Other
- exposing definition of merkletree nodes data type and additional doc in code
- making Chunk payment proof optional for now
- moving all payment proofs utilities into sn_transfers crate

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
