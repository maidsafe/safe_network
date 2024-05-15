# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2](https://github.com/maidsafe/safe_network/compare/node-launchpad-v0.1.1...node-launchpad-v0.1.2) - 2024-05-15

### Added
- *(launchpad)* initial automatic resource allocation logic
- *(launchpad)* allow users to input disk space to allocate
- *(launchpad)* store discord username to disk
- *(launchpad)* use escape to exit input screen and restore old value
- *(launchpad)* have customizable footer
- *(launchpad)* add discord username scene

### Fixed
- *(launchpad)* check if component is active before handling events
- *(launchpad)* prevent mac opening with sudo
- *(launchpad)* prevent loops from terminal/sudo relaunching
- use fixed size popups

### Other
- *(launchpad)* removing redudnat for loops
- move helper text inside popup
- change trigger resource allocation input box keybind
- *(launchpad)* highlight the table in green if we're currently running
- *(launchpad)* add more alternative keybinds
- change terminal launch behaviour
- use consistent border styles
- *(launchpad)* use safe data dir to store configs

## [0.1.1](https://github.com/maidsafe/safe_network/compare/node-launchpad-v0.1.0...node-launchpad-v0.1.1) - 2024-05-08

### Other
- update Cargo.lock dependencies

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
