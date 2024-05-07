# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/maidsafe/safe_network/releases/tag/node-launchpad-v0.1.0) - 2024-05-07

### Added
- *(launchpad)* remove separate ai launcher bin references
- *(launchpad)* ensure start mac launchapd with sudo only if not set

### Fixed
- *(launchpad)* do not try to run sudo twice

### Other
- *(versions)* sync versions with latest crates.io vs for nodeman
- *(versions)* sync versions with latest crates.io vs
- rename sn_node_launchpad -> node-launchpad
- rename `node-launchpad` crate to `sn_node_launchpad`
- rebased and removed custom rustfmt
- *(tui)* rename crate
