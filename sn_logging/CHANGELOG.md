# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.6](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.5...sn_logging-v0.2.6) - 2023-09-06

### Other
- rotate logs after exceeding 20mb

## [0.2.5](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.4...sn_logging-v0.2.5) - 2023-08-30

### Other
- *(deps)* bump tokio to 1.32.0

## [0.2.4](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.3...sn_logging-v0.2.4) - 2023-08-17

### Fixed
- *(logging)* get log name per bin

## [0.2.3](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.2...sn_logging-v0.2.3) - 2023-07-20

### Other
- cleanup error types

## [0.2.2](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.1...sn_logging-v0.2.2) - 2023-07-13

### Other
- *(clippy)* fix clippy warnings

## [0.2.1](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.0...sn_logging-v0.2.1) - 2023-07-13

### Other
- *(metrics)* remove network stats

## [0.2.0](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.1.5...sn_logging-v0.2.0) - 2023-07-06

### Added
- introduce `--log-format` arguments
- provide `--log-output-dest` arg for `safe`
- provide `--log-output-dest` arg for `safenode`

## [0.1.5](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.1.4...sn_logging-v0.1.5) - 2023-07-05

### Added
- carry out validation for record_store::put

## [0.1.4](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.1.3...sn_logging-v0.1.4) - 2023-06-26

### Other
- *(logging)* dont log PID with metrics

## [0.1.3](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.1.2...sn_logging-v0.1.3) - 2023-06-21

### Added
- provide option for log output in json

## [0.1.2](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.1.1...sn_logging-v0.1.2) - 2023-06-13

### Added
- *(node)* log PID of node w/ metrics in debug

## [0.1.1](https://github.com/jacderida/safe_network/compare/sn_logging-v0.1.0...sn_logging-v0.1.1) - 2023-06-06

### Added
- *(logging)* log metrics for safe and safenode bin

## [0.1.0](https://github.com/jacderida/safe_network/releases/tag/sn_logging-v0.1.0) - 2023-06-04

### Added
- add registers and transfers crates, deprecate domain
- *(logs)* add 'all' log shorthand
- add build_info crate

### Fixed
- add missing safenode/safe trace to  logs
- local-discovery deps
- remove unused deps, fix doc comment

### Other
- accommodate new workspace
- extract logging and networking crates
