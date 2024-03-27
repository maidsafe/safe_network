# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.12](https://github.com/joshuef/safe_network/compare/sn_registers-v0.3.11...sn_registers-v0.3.12) - 2024-03-27

### Fixed
- *(register)* shortcut permissions check when anyone can write to Register
- *(register)* permissions verification was not being made by some Register APIs

### Other
- *(uploader)* initial test setup for uploader
- *(register)* minor simplification in Register Permissions implementation

## [0.3.10](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.3.9...sn_registers-v0.3.10) - 2024-02-20

### Added
- *(registers)* expose MerkleReg of RegisterCrdt in all Register types

### Fixed
- cargo fmt changes
- clippy warnings

### Other
- marke merkle_reg() accessors as unstable (in comment) on Register types

## [0.3.9](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.3.8...sn_registers-v0.3.9) - 2024-02-08

### Other
- copyright update to current year

## [0.3.8](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.3.7...sn_registers-v0.3.8) - 2024-01-24

### Added
- remove registers self_encryption dep

## [0.3.7](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.3.6...sn_registers-v0.3.7) - 2024-01-11

### Fixed
- update MAX_REG_ENTRY_SIZE

### Other
- udpate self_encryption dep

## [0.3.6](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.3.5...sn_registers-v0.3.6) - 2023-12-14

### Other
- *(protocol)* print the first six hex characters for every address type

## [0.3.5](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.3.4...sn_registers-v0.3.5) - 2023-12-06

### Other
- remove some needless cloning
- remove needless pass by value
- use inline format args
- add boilerplate for workspace lints

## [0.3.4](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.3.3...sn_registers-v0.3.4) - 2023-11-28

### Added
- *(registers)* serialise Registers for signing with MsgPack instead of bincode

## [0.3.3](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.3.2...sn_registers-v0.3.3) - 2023-10-26

### Fixed
- typos

## [0.3.2](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.3.1...sn_registers-v0.3.2) - 2023-10-20

### Fixed
- RegisterAddress logging with correct network addressing

## [0.3.1](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.3.0...sn_registers-v0.3.1) - 2023-10-10

### Other
- compare files after download twice

## [0.3.0](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.2.6...sn_registers-v0.3.0) - 2023-10-04

### Added
- improve register API

### Other
- fix name discrepancy

## [0.2.6](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.2.5...sn_registers-v0.2.6) - 2023-09-11

### Other
- utilize stream encryptor

## [0.2.5](https://github.com/maidsafe/safe_network/compare/sn_registers-v0.2.4...sn_registers-v0.2.5) - 2023-09-05

### Added
- encryptioni output to disk

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
