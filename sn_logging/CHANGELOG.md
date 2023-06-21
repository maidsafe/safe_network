# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
