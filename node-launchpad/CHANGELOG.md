# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.5](https://github.com/maidsafe/safe_network/compare/node-launchpad-v0.1.4...node-launchpad-v0.1.5) - 2024-05-20

### Added
- *(node_manager)* add auditor support
- provide `--upnp` flag for `add` command

### Fixed
- retain options on upgrade and prevent dup ports

### Other
- use published versions of deps
- update Cargo.lock dependencies
- use helper function to print banners

## [0.1.4](https://github.com/maidsafe/safe_network/compare/node-launchpad-v0.1.3...node-launchpad-v0.1.4) - 2024-05-17

### Added
- *(node-launchpad)* discord name widget styling
- *(node-launchpad)* tweaks on resource allocation widget

## [0.1.3](https://github.com/maidsafe/safe_network/compare/node-launchpad-v0.1.2...node-launchpad-v0.1.3) - 2024-05-15

### Added
- *(launchpad)* initial automatic resource allocation logic
- run safenode services in user mode

### Other
- *(release)* sn_auditor-v0.1.15/sn_cli-v0.91.3/sn_faucet-v0.4.17/sn_metrics-v0.1.6/sn_node-v0.106.3/sn_service_management-v0.2.7/node-launchpad-v0.1.2/sn_node_rpc_client-v0.6.16
- change terminal launch behaviour
- update cli and readme for user-mode services
- upgrade service manager crate
- *(release)* sn_auditor-v0.1.13/sn_client-v0.106.1/sn_networking-v0.15.1/sn_protocol-v0.16.6/sn_cli-v0.91.1/sn_faucet-v0.4.15/sn_node-v0.106.1/node-launchpad-v0.1.1/sn_node_rpc_client-v0.6.14/sn_peers_acquisition-v0.2.12/sn_service_management-v0.2.6

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
