# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/joshuef/safe_network/releases/tag/sn_service_management-v0.1.0) - 2024-03-14

### Added
- add rpc to fetch status from the daemon

### Fixed
- *(manager)* don't error out when fetching pid for the daemon

### Other
- *(service)* remove the node service restart workaround
- extend `status` cmd for faucet and daemon
- correctly run node manager unit tests
- move rpc to its own module
- [**breaking**] uniform service management
- new `sn_service_management` crate
