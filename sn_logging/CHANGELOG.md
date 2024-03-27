# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.25](https://github.com/joshuef/safe_network/compare/sn_logging-v0.2.24...sn_logging-v0.2.25) - 2024-03-27

### Added
- make logging simpler to use

### Fixed
- logging, adapt program name

### Other
- fix typo
- adapt client name for safe cli cmd

## [0.2.24](https://github.com/joshuef/safe_network/compare/sn_logging-v0.2.23...sn_logging-v0.2.24) - 2024-03-21

### Added
- *(log)* set log levels on the fly

### Other
- *(log)* add test to verify log reload functionality

## [0.2.23](https://github.com/joshuef/safe_network/compare/sn_logging-v0.2.22...sn_logging-v0.2.23) - 2024-03-14

### Other
- *(api)* make logging::Error public

## [0.2.21](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.20...sn_logging-v0.2.21) - 2024-02-08

### Other
- copyright update to current year

## [0.2.20](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.19...sn_logging-v0.2.20) - 2024-02-08

### Other
- Revert "chore: roll back to log more"

## [0.2.19](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.18...sn_logging-v0.2.19) - 2024-02-06

### Other
- roll back to log more

## [0.2.18](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.17...sn_logging-v0.2.18) - 2024-01-31

### Other
- remove the `sn_testnet` crate

## [0.2.17](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.16...sn_logging-v0.2.17) - 2024-01-23

### Other
- *(node)* reduce MAX_UNCOMPRESSED_LOG_FILES to 10

## [0.2.16](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.15...sn_logging-v0.2.16) - 2023-12-06

### Other
- remove needless pass by value
- use inline format args
- add boilerplate for workspace lints
- address failing clippy::all lints

## [0.2.15](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.14...sn_logging-v0.2.15) - 2023-11-21

### Fixed
- *(logs)* enable faucet logs

## [0.2.14](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.13...sn_logging-v0.2.14) - 2023-10-26

### Fixed
- typos

## [0.2.13](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.12...sn_logging-v0.2.13) - 2023-10-24

### Added
- *(log)* use LogBuilder to initialize logging

## [0.2.12](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.11...sn_logging-v0.2.12) - 2023-10-23

### Other
- more custom debug and debug skips

## [0.2.11](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.10...sn_logging-v0.2.11) - 2023-10-11

### Fixed
- *(log)* capture logs from multiple integration tests
- *(log)* capture logs from tests

## [0.2.10](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.9...sn_logging-v0.2.10) - 2023-10-03

### Other
- *(logging)* reduce metric frequency and logged stats.

## [0.2.9](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.8...sn_logging-v0.2.9) - 2023-09-20

### Other
- major dep updates

## [0.2.8](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.7...sn_logging-v0.2.8) - 2023-09-15

### Added
- *(logging)* Add in SN_LOG=v for reduced networking logging

## [0.2.7](https://github.com/maidsafe/safe_network/compare/sn_logging-v0.2.6...sn_logging-v0.2.7) - 2023-09-14

### Other
- remove unused error variants

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
