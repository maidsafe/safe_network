# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.4](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.2.3...sn_registers-v0.2.4) - 2023-09-04

### Other
- utilize encrypt_from_file

## [0.2.3](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.2.2...sn_registers-v0.2.3) - 2023-08-24

### Other
- rust 1.72.0 fixes

## [0.2.2](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.2.1...sn_registers-v0.2.2) - 2023-08-07

### Added
- rework register addresses to include pk

### Fixed
- signature issue when owner was not signer

### Other
- rename network addresses confusing name method to xorname
- cleanup comments and names

## [0.2.1](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.2.0...sn_registers-v0.2.1) - 2023-08-01

### Fixed
- relay attacks

## [0.2.0](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.1.11...sn_registers-v0.2.0) - 2023-08-01

### Other
- *(register)* [**breaking**] hashing the node of a Register to sign it instead of bincode-serialising it

## [0.1.11](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.1.10...sn_registers-v0.1.11) - 2023-07-18

### Added
- safer registers requiring signatures

### Fixed
- address PR comments

## [0.1.10](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.1.9...sn_registers-v0.1.10) - 2023-07-04

### Fixed
- perm test

### Other
- demystify permissions

## [0.1.9](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.1.8...sn_registers-v0.1.9) - 2023-06-28

### Added
- make the example work, fix sync when reg doesnt exist
- rework permissions, implement register cmd handlers
- register refactor, kad reg without cmds

### Fixed
- rename UserRights to UserPermissions
- permission in test

### Other
- bypass crypto in test with lax permissions

## [0.1.8](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.1.7...sn_registers-v0.1.8) - 2023-06-21

### Other
- updated the following local packages: sn_protocol

## [0.1.7](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.1.6...sn_registers-v0.1.7) - 2023-06-21

### Other
- updated the following local packages: sn_protocol

## [0.1.6](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.1.5...sn_registers-v0.1.6) - 2023-06-20

### Other
- updated the following local packages: sn_protocol

## [0.1.5](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.1.4...sn_registers-v0.1.5) - 2023-06-20

### Other
- updated the following local packages: sn_protocol

## [0.1.4](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.1.3...sn_registers-v0.1.4) - 2023-06-15

### Other
- updated the following local packages: sn_protocol

## [0.1.3](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.1.2...sn_registers-v0.1.3) - 2023-06-14

### Other
- updated the following local packages: sn_protocol

## [0.1.1](https://github.com/jacderida/safe_network/compare/sn_registers-v0.1.0...sn_registers-v0.1.1) - 2023-06-06

### Other
- updated the following local packages: sn_protocol

## [0.1.0](https://github.com/jacderida/safe_network/releases/tag/sn_registers-v0.1.0) - 2023-06-04

### Added
- add registers and transfers crates, deprecate domain
