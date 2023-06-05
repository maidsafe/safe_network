# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
