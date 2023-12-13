# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.8](https://github.com/maidsafe/sn-node-manager/compare/v0.1.7...v0.1.8) - 2023-12-13

### Other
- remove network contacts from peer acquisition

## [0.1.7](https://github.com/maidsafe/sn-node-manager/compare/v0.1.6...v0.1.7) - 2023-12-13

### Added
- provide `--url` argument for `add` command

## [0.1.6](https://github.com/maidsafe/sn-node-manager/compare/v0.1.5...v0.1.6) - 2023-12-12

### Fixed
- accommodate service restarts in `status` cmd

## [0.1.5](https://github.com/maidsafe/sn-node-manager/compare/v0.1.4...v0.1.5) - 2023-12-08

### Added
- provide `upgrade` command
- each service instance to use its own binary

## [0.1.4](https://github.com/maidsafe/sn-node-manager/compare/v0.1.3...v0.1.4) - 2023-12-05

### Other
- upload 'latest' version to S3

## [0.1.3](https://github.com/maidsafe/sn-node-manager/compare/v0.1.2...v0.1.3) - 2023-12-05

### Added
- provide `remove` command

## [0.1.2](https://github.com/maidsafe/sn-node-manager/compare/v0.1.1...v0.1.2) - 2023-12-05

### Added
- provide `--peer` argument

### Other
- rename `install` command to `add`

## [0.1.1](https://github.com/maidsafe/sn-node-manager/compare/v0.1.0...v0.1.1) - 2023-11-29

### Other
- improve docs for `start` and `stop` commands

## [0.1.0](https://github.com/maidsafe/sn-node-manager/releases/tag/v0.1.0) - 2023-11-29

### Added
- provide `status` command
- provide `stop` command
- provide `start` command
- provide `install` command

### Other
- release process and licensing
- extend the e2e test for new commands
- reference `sn_node_rpc_client` crate
- specify root and log dirs at install time
- provide initial integration tests
- Initial commit
