# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.17.4](https://github.com/joshuef/safe_network/compare/sn_protocol-v0.17.3...sn_protocol-v0.17.4) - 2024-06-04

### Other
- release
- release

## [0.17.3](https://github.com/joshuef/safe_network/compare/sn_protocol-v0.17.2...sn_protocol-v0.17.3) - 2024-06-04

### Other
- updated the following local packages: sn_transfers

## [0.17.2](https://github.com/joshuef/safe_network/compare/sn_protocol-v0.17.1...sn_protocol-v0.17.2) - 2024-06-03

### Other
- updated the following local packages: sn_transfers

## [0.17.0](https://github.com/joshuef/safe_network/compare/sn_protocol-v0.16.7...sn_protocol-v0.17.0) - 2024-06-03

### Added
- *(network)* [**breaking**] move network versioning away from sn_protocol

## [0.16.7](https://github.com/joshuef/safe_network/compare/sn_protocol-v0.16.6...sn_protocol-v0.16.7) - 2024-05-24

### Other
- updated the following local packages: sn_transfers

## [0.16.6](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.16.5...sn_protocol-v0.16.6) - 2024-05-08

### Other
- *(release)* sn_registers-v0.3.13

## [0.16.5](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.16.4-alpha.0...sn_protocol-v0.16.5) - 2024-05-07

### Other
- updated the following local packages: sn_transfers

## [0.16.1](https://github.com/joshuef/safe_network/compare/sn_protocol-v0.16.0...sn_protocol-v0.16.1) - 2024-03-28

### Other
- updated the following local packages: sn_transfers

## [0.16.0](https://github.com/joshuef/safe_network/compare/sn_protocol-v0.15.5...sn_protocol-v0.16.0) - 2024-03-27

### Added
- [**breaking**] remove gossip code

## [0.15.5](https://github.com/joshuef/safe_network/compare/sn_protocol-v0.15.4...sn_protocol-v0.15.5) - 2024-03-21

### Added
- *(protocol)* add rpc to set node log level on the fly

## [0.15.4](https://github.com/joshuef/safe_network/compare/sn_protocol-v0.15.3...sn_protocol-v0.15.4) - 2024-03-14

### Fixed
- dont stop spend verification at spend error, generalise spend serde

### Other
- store test utils under a new crate
- move DeploymentInventory to test utils
- new `sn_service_management` crate
- *(release)* sn_transfers-v0.16.3/sn_cli-v0.89.82

## [0.15.3](https://github.com/joshuef/safe_network/compare/sn_protocol-v0.15.2-alpha.0...sn_protocol-v0.15.3) - 2024-03-08

### Other
- updated the following local packages: sn_transfers

## [0.15.1](https://github.com/joshuef/safe_network/compare/sn_protocol-v0.15.0...sn_protocol-v0.15.1) - 2024-03-06

### Other
- *(release)* sn_transfers-v0.16.1

## [0.15.0](https://github.com/joshuef/safe_network/compare/sn_protocol-v0.14.8...sn_protocol-v0.15.0) - 2024-03-05

### Added
- *(node)* bad verification to exclude connections from bad_nodes
- *(manager)* add subcommands for daemon
- *(test)* add option to retain_peer_id for the node's restart rpc cmd
- *(test)* imporve restart api for tests
- *(protocol)* add daemon socket addr to node registry
- *(manager)* add rpc call to restart node service and process
- [**breaking**] provide `faucet start` command
- provide `faucet add` command

### Other
- *(daemon)* rename daemon binary to safenodemand
- *(manager)* removing support for process restarts

## [0.14.8](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.14.7...sn_protocol-v0.14.8) - 2024-02-23

### Other
- updated the following local packages: sn_transfers

## [0.14.7](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.14.6...sn_protocol-v0.14.7) - 2024-02-21

### Other
- *(release)* initial alpha test release

## [0.14.6](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.14.5...sn_protocol-v0.14.6) - 2024-02-20

### Added
- *(manager)* setup initial bin for safenode mangaer daemon

## [0.14.5](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.14.4...sn_protocol-v0.14.5) - 2024-02-20

### Other
- updated the following local packages: sn_transfers

## [0.14.4](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.14.3...sn_protocol-v0.14.4) - 2024-02-20

### Other
- updated the following local packages: sn_transfers

## [0.14.3](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.14.2...sn_protocol-v0.14.3) - 2024-02-20

### Other
- updated the following local packages: sn_registers

## [0.14.2](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.14.1...sn_protocol-v0.14.2) - 2024-02-15

### Other
- updated the following local packages: sn_transfers

## [0.14.1](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.14.0...sn_protocol-v0.14.1) - 2024-02-15

### Added
- force and upgrade by url or version

## [0.14.0](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.13.1...sn_protocol-v0.14.0) - 2024-02-14

### Added
- *(manager)* [**breaking**] store the env variables inside the NodeRegistry

## [0.13.1](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.13.0...sn_protocol-v0.13.1) - 2024-02-14

### Other
- *(refactor)* move mod.rs files the modern way

## [0.13.0](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.12.7...sn_protocol-v0.13.0) - 2024-02-13

### Added
- *(protocol)* include local flag inside registry's Node struct
- *(protocol)* obtain safenode's port from listen addr
- *(sn_protocol)* [**breaking**] store the bootstrap peers inside the NodeRegistry

### Other
- *(protocol)* [**breaking**] make node dirs not optional

## [0.12.7](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.12.6...sn_protocol-v0.12.7) - 2024-02-13

### Other
- updated the following local packages: sn_transfers

## [0.12.6](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.12.5...sn_protocol-v0.12.6) - 2024-02-08

### Other
- copyright update to current year

## [0.12.5](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.12.4...sn_protocol-v0.12.5) - 2024-02-08

### Added
- move the RetryStrategy into protocol and use that during cli upload/download

## [0.12.4](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.12.3...sn_protocol-v0.12.4) - 2024-02-07

### Other
- updated the following local packages: sn_transfers

## [0.12.3](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.12.2...sn_protocol-v0.12.3) - 2024-02-06

### Other
- updated the following local packages: sn_transfers

## [0.12.2](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.12.1...sn_protocol-v0.12.2) - 2024-02-05

### Fixed
- node manager `status` permissions error

## [0.12.1](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.12.0...sn_protocol-v0.12.1) - 2024-02-02

### Other
- updated the following local packages: sn_transfers

## [0.12.0](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.11.3...sn_protocol-v0.12.0) - 2024-01-31

### Other
- *(protocol)* [**breaking**] remove node's port from NodeRegistry

## [0.11.3](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.11.2...sn_protocol-v0.11.3) - 2024-01-30

### Other
- *(manager)* provide rpc address instead of rpc port

## [0.11.2](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.11.1...sn_protocol-v0.11.2) - 2024-01-29

### Other
- updated the following local packages: sn_transfers

## [0.11.1](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.11.0...sn_protocol-v0.11.1) - 2024-01-25

### Added
- client webtransport-websys feat

## [0.11.0](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.10.14...sn_protocol-v0.11.0) - 2024-01-24

### Added
- make RPC portions or protocol a feature
- client webtransport-websys feat

## [0.10.14](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.10.13...sn_protocol-v0.10.14) - 2024-01-22

### Fixed
- create parent directories

### Other
- include connected peers in node

## [0.10.13](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.10.12...sn_protocol-v0.10.13) - 2024-01-22

### Other
- updated the following local packages: sn_transfers

## [0.10.12](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.10.11...sn_protocol-v0.10.12) - 2024-01-18

### Added
- *(rpc)* add wallet balance to NodeInfo response

## [0.10.11](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.10.10...sn_protocol-v0.10.11) - 2024-01-18

### Added
- set quic as default transport

## [0.10.10](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.10.9...sn_protocol-v0.10.10) - 2024-01-18

### Other
- updated the following local packages: sn_transfers

## [0.10.9](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.10.8...sn_protocol-v0.10.9) - 2024-01-16

### Other
- updated the following local packages: sn_transfers

## [0.10.8](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.10.7...sn_protocol-v0.10.8) - 2024-01-15

### Other
- use node manager for running local testnets

## [0.10.7](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.10.6...sn_protocol-v0.10.7) - 2024-01-15

### Other
- updated the following local packages: sn_transfers

## [0.10.6](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.10.5...sn_protocol-v0.10.6) - 2024-01-11

### Other
- updated the following local packages: sn_registers

## [0.10.5](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.10.4...sn_protocol-v0.10.5) - 2024-01-10

### Other
- updated the following local packages: sn_transfers

## [0.10.4](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.10.3...sn_protocol-v0.10.4) - 2024-01-09

### Other
- updated the following local packages: sn_transfers

## [0.10.3](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.10.2...sn_protocol-v0.10.3) - 2024-01-09

### Other
- *(node)* move add_to_replicate_fetcher to driver

## [0.10.2](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.10.1...sn_protocol-v0.10.2) - 2024-01-08

### Other
- updated the following local packages: sn_transfers

## [0.10.1](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.10.0...sn_protocol-v0.10.1) - 2024-01-05

### Fixed
- ignore unwraps in protogen files

## [0.10.0](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.9.4...sn_protocol-v0.10.0) - 2023-12-28

### Added
- *(protocol)* [**breaking**] new request response for ChunkExistenceProof

## [0.9.4](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.9.3...sn_protocol-v0.9.4) - 2023-12-19

### Other
- add data path field to node info

## [0.9.3](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.9.2...sn_protocol-v0.9.3) - 2023-12-18

### Other
- updated the following local packages: sn_transfers

## [0.9.2](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.9.1...sn_protocol-v0.9.2) - 2023-12-14

### Other
- *(protocol)* print the first six hex characters for every address type

## [0.9.1](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.9.0...sn_protocol-v0.9.1) - 2023-12-12

### Fixed
- reduce duplicated kbucket part when logging NetworkAddress::RecordKey

## [0.9.0](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.39...sn_protocol-v0.9.0) - 2023-12-12

### Added
- *(networking)* sort quotes by closest NetworkAddress before truncate

## [0.8.39](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.38...sn_protocol-v0.8.39) - 2023-12-06

### Other
- updated the following local packages: sn_transfers

## [0.8.38](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.37...sn_protocol-v0.8.38) - 2023-12-06

### Other
- use inline format args
- add boilerplate for workspace lints
- address failing clippy::all lints

## [0.8.37](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.36...sn_protocol-v0.8.37) - 2023-12-05

### Other
- *(network)* avoid losing error info by converting them to a single type

## [0.8.36](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.35...sn_protocol-v0.8.36) - 2023-12-05

### Other
- updated the following local packages: sn_transfers

## [0.8.35](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.34...sn_protocol-v0.8.35) - 2023-12-05

### Other
- improve Replication debug

## [0.8.34](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.33...sn_protocol-v0.8.34) - 2023-12-01

### Added
- *(network)* use seperate PUT/GET configs

### Other
- *(ci)* fix CI build cache parsing error

## [0.8.33](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.32...sn_protocol-v0.8.33) - 2023-11-29

### Added
- verify spends through the cli

## [0.8.32](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.31...sn_protocol-v0.8.32) - 2023-11-28

### Other
- updated the following local packages: sn_registers, sn_transfers

## [0.8.31](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.30...sn_protocol-v0.8.31) - 2023-11-28

### Added
- *(test)* impl more functions for deployer tests

### Other
- *(test)* impl utils for Droplets/NonDroplets

## [0.8.30](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.29...sn_protocol-v0.8.30) - 2023-11-27

### Added
- *(rpc)* return the KBuckets map

## [0.8.29](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.28...sn_protocol-v0.8.29) - 2023-11-23

### Other
- updated the following local packages: sn_transfers

## [0.8.28](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.27...sn_protocol-v0.8.28) - 2023-11-22

### Other
- updated the following local packages: sn_transfers

## [0.8.27](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.26...sn_protocol-v0.8.27) - 2023-11-20

### Other
- updated the following local packages: sn_transfers

## [0.8.26](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.25...sn_protocol-v0.8.26) - 2023-11-20

### Added
- quotes

## [0.8.25](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.24...sn_protocol-v0.8.25) - 2023-11-16

### Other
- updated the following local packages: sn_transfers

## [0.8.24](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.23...sn_protocol-v0.8.24) - 2023-11-15

### Other
- include RPC endpoints field to DeploymentInventory

## [0.8.23](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.22...sn_protocol-v0.8.23) - 2023-11-15

### Added
- *(test)* read the DeploymentInventory from SN_INVENTORY
- *(protocol)* move test utils behind a feature gate

## [0.8.22](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.21...sn_protocol-v0.8.22) - 2023-11-14

### Other
- updated the following local packages: sn_transfers

## [0.8.21](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.20...sn_protocol-v0.8.21) - 2023-11-10

### Other
- updated the following local packages: sn_transfers

## [0.8.20](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.19...sn_protocol-v0.8.20) - 2023-11-10

### Other
- mutable_key_type clippy fixes
- *(networking)* sort records by closeness

## [0.8.19](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.18...sn_protocol-v0.8.19) - 2023-11-09

### Other
- updated the following local packages: sn_transfers

## [0.8.18](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.17...sn_protocol-v0.8.18) - 2023-11-08

### Added
- *(node)* set custom msg id in order to deduplicate transfer notifs

## [0.8.17](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.16...sn_protocol-v0.8.17) - 2023-11-07

### Fixed
- do not allocate while serializing PrettyPrintRecordKey

### Other
- rename test function and spell correction
- *(cli)* add more tests to chunk manager for unpaid paid dir refactor
- *(cli)* add tests for `ChunkManager`

## [0.8.16](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.15...sn_protocol-v0.8.16) - 2023-11-07

### Other
- move protobuf definition to sn_protocol

## [0.8.15](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.14...sn_protocol-v0.8.15) - 2023-11-06

### Other
- *(protocol)* use exposed hashed_bytes method

## [0.8.14](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.13...sn_protocol-v0.8.14) - 2023-11-06

### Other
- using libp2p newly exposed API to avoid hash work

## [0.8.13](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.12...sn_protocol-v0.8.13) - 2023-11-06

### Added
- *(deps)* upgrade libp2p to 0.53

## [0.8.12](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.11...sn_protocol-v0.8.12) - 2023-11-02

### Other
- updated the following local packages: sn_transfers

## [0.8.11](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.10...sn_protocol-v0.8.11) - 2023-11-01

### Other
- *(networking)* make NetworkAddress hold bytes rather than vec<u8>

## [0.8.10](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.9...sn_protocol-v0.8.10) - 2023-11-01

### Other
- updated the following local packages: sn_transfers

## [0.8.9](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.8...sn_protocol-v0.8.9) - 2023-10-30

### Other
- *(networking)* de/serialise directly to Bytes

## [0.8.8](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.7...sn_protocol-v0.8.8) - 2023-10-30

### Other
- updated the following local packages: sn_transfers

## [0.8.7](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.6...sn_protocol-v0.8.7) - 2023-10-27

### Added
- encrypt network royalty to Transfer for gossip msg

## [0.8.6](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.5...sn_protocol-v0.8.6) - 2023-10-26

### Added
- replicate Spend/Register with same key but different content

## [0.8.5](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.4...sn_protocol-v0.8.5) - 2023-10-26

### Other
- updated the following local packages: sn_registers, sn_transfers

## [0.8.4](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.3...sn_protocol-v0.8.4) - 2023-10-26

### Other
- pass RecordKey by reference

## [0.8.3](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.2...sn_protocol-v0.8.3) - 2023-10-24

### Other
- updated the following local packages: sn_transfers

## [0.8.2](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.1...sn_protocol-v0.8.2) - 2023-10-24

### Added
- *(payments)* network royalties payment made when storing content

### Fixed
- *(node)* include network royalties in received fee calculation

## [0.8.1](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.8.0...sn_protocol-v0.8.1) - 2023-10-24

### Other
- updated the following local packages: sn_transfers

## [0.8.0](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.28...sn_protocol-v0.8.0) - 2023-10-24

### Added
- *(protocol)* remove allocation inside `PrettyPrintRecordKey::Display`
- *(protocol)* [**breaking**] implement `PrettyPrintRecordKey` as a `Cow` type

### Fixed
- *(protocol)* use custom `Display` for `PrettyPrintKBucketKey`

## [0.7.28](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.27...sn_protocol-v0.7.28) - 2023-10-23

### Fixed
- *(protocol)* add custom debug fmt for QueryResponse

### Other
- more custom debug and debug skips

## [0.7.27](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.26...sn_protocol-v0.7.27) - 2023-10-22

### Added
- *(protocol)* Nodes can error StoreCosts if they have data.

## [0.7.26](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.25...sn_protocol-v0.7.26) - 2023-10-20

### Added
- log network address with KBucketKey

## [0.7.25](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.24...sn_protocol-v0.7.25) - 2023-10-20

### Other
- print the PeerId along with the raw bytes

## [0.7.24](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.23...sn_protocol-v0.7.24) - 2023-10-18

### Other
- updated the following local packages: sn_transfers

## [0.7.23](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.22...sn_protocol-v0.7.23) - 2023-10-18

### Other
- updated the following local packages: sn_transfers

## [0.7.22](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.21...sn_protocol-v0.7.22) - 2023-10-17

### Other
- updated the following local packages: sn_transfers

## [0.7.21](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.20...sn_protocol-v0.7.21) - 2023-10-13

### Fixed
- *(network)* check `RecordHeader` during chunk early completion

## [0.7.20](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.19...sn_protocol-v0.7.20) - 2023-10-12

### Other
- updated the following local packages: sn_transfers

## [0.7.19](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.18...sn_protocol-v0.7.19) - 2023-10-11

### Other
- updated the following local packages: sn_transfers

## [0.7.18](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.17...sn_protocol-v0.7.18) - 2023-10-10

### Other
- updated the following local packages: sn_transfers

## [0.7.17](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.16...sn_protocol-v0.7.17) - 2023-10-10

### Other
- updated the following local packages: sn_registers

## [0.7.16](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.15...sn_protocol-v0.7.16) - 2023-10-10

### Other
- updated the following local packages: sn_transfers

## [0.7.15](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.14...sn_protocol-v0.7.15) - 2023-10-06

### Other
- updated the following local packages: sn_transfers

## [0.7.14](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.13...sn_protocol-v0.7.14) - 2023-10-06

### Other
- updated the following local packages: sn_transfers

## [0.7.13](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.12...sn_protocol-v0.7.13) - 2023-10-05

### Other
- updated the following local packages: sn_transfers

## [0.7.12](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.11...sn_protocol-v0.7.12) - 2023-10-05

### Other
- updated the following local packages: sn_transfers

## [0.7.11](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.10...sn_protocol-v0.7.11) - 2023-10-05

### Other
- updated the following local packages: sn_transfers

## [0.7.10](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.9...sn_protocol-v0.7.10) - 2023-10-05

### Other
- updated the following local packages: sn_transfers

## [0.7.9](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.8...sn_protocol-v0.7.9) - 2023-10-04

### Other
- updated the following local packages: sn_registers

## [0.7.8](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.7...sn_protocol-v0.7.8) - 2023-10-04

### Other
- updated the following local packages: sn_transfers

## [0.7.7](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.6...sn_protocol-v0.7.7) - 2023-10-02

### Other
- updated the following local packages: sn_transfers

## [0.7.6](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.5...sn_protocol-v0.7.6) - 2023-09-29

### Added
- replicate fetch from peer first then from network

## [0.7.5](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.4...sn_protocol-v0.7.5) - 2023-09-28

### Other
- updated the following local packages: sn_transfers

## [0.7.4](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.3...sn_protocol-v0.7.4) - 2023-09-27

### Other
- updated the following local packages: sn_transfers

## [0.7.3](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.2...sn_protocol-v0.7.3) - 2023-09-25

### Other
- updated the following local packages: sn_transfers

## [0.7.2](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.1...sn_protocol-v0.7.2) - 2023-09-25

### Other
- updated the following local packages: sn_transfers

## [0.7.1](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.7.0...sn_protocol-v0.7.1) - 2023-09-22

### Other
- *(gossipsub)* CI testing with nodes subscribing to gossipsub topics and publishing messages

## [0.7.0](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.6.10...sn_protocol-v0.7.0) - 2023-09-21

### Added
- dusking DBCs

### Other
- remove dbc dust comments
- rename Nano NanoTokens

## [0.6.10](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.6.9...sn_protocol-v0.6.10) - 2023-09-18

### Added
- generic transfer receipt

## [0.6.9](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.6.8...sn_protocol-v0.6.9) - 2023-09-14

### Other
- remove unused error variants

## [0.6.8](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.6.7...sn_protocol-v0.6.8) - 2023-09-13

### Added
- *(register)* paying nodes for Register storage

## [0.6.7](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.6.6...sn_protocol-v0.6.7) - 2023-09-12

### Added
- add tx and parent spends verification
- chunk payments using UTXOs instead of DBCs

### Other
- use updated sn_dbc

## [0.6.6](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.6.5...sn_protocol-v0.6.6) - 2023-09-11

### Other
- updated the following local packages: sn_registers

## [0.6.5](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.6.4...sn_protocol-v0.6.5) - 2023-09-05

### Other
- updated the following local packages: sn_registers

## [0.6.4](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.6.3...sn_protocol-v0.6.4) - 2023-09-04

### Added
- feat!(protocol): make payments for all record types

### Other
- *(release)* sn_registers-v0.2.4
- add RegisterWithSpend header validation
- se/derialize for PrettyPrintRecordKey

## [0.6.3](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.6.2...sn_protocol-v0.6.3) - 2023-09-04

### Other
- Add client and protocol detail

## [0.6.2](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.6.1...sn_protocol-v0.6.2) - 2023-08-31

### Added
- *(node)* node to store rewards in a local wallet

## [0.6.1](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.6.0...sn_protocol-v0.6.1) - 2023-08-31

### Added
- fetch from network during network

## [0.6.0](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.5.3...sn_protocol-v0.6.0) - 2023-08-30

### Added
- *(protocol)* add logs for `RecordHeader` serde
- one transfer per data set, mapped dbcs to content addrs
- [**breaking**] pay each chunk holder direct
- feat!(protocol): gets keys with GetStoreCost
- feat!(protocol): get price and pay for each chunk individually
- feat!(protocol): remove chunk merkletree to simplify payment

### Fixed
- *(protocol)* avoid panics

### Other
- *(node)* data verification test refactors for readability
- *(node)* only store paid for data, ignore maj
- *(node)* clarify payment errors
- *(node)* reenable payment fail check

## [0.5.3](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.5.2...sn_protocol-v0.5.3) - 2023-08-24

### Other
- updated the following local packages: sn_registers

## [0.5.2](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.5.1...sn_protocol-v0.5.2) - 2023-08-18

### Added
- UTXO and Transfer

## [0.5.1](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.5.0...sn_protocol-v0.5.1) - 2023-08-10

### Fixed
- *(test)* have multiple verification attempts

## [0.5.0](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.4.6...sn_protocol-v0.5.0) - 2023-08-08

### Added
- *(node)* validate payments on kad:put

## [0.4.6](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.4.5...sn_protocol-v0.4.6) - 2023-08-08

### Added
- *(networking)* remove sign over store cost

## [0.4.5](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.4.4...sn_protocol-v0.4.5) - 2023-08-07

### Added
- rework register addresses to include pk

### Other
- rename network addresses confusing name method to xorname

## [0.4.4](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.4.3...sn_protocol-v0.4.4) - 2023-08-01

### Other
- updated the following local packages: sn_registers

## [0.4.3](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.4.2...sn_protocol-v0.4.3) - 2023-08-01

### Other
- cleanup old dead API

## [0.4.2](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.4.1...sn_protocol-v0.4.2) - 2023-08-01

### Other
- updated the following local packages: sn_registers

## [0.4.1](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.4.0...sn_protocol-v0.4.1) - 2023-07-31

### Other
- move PrettyPrintRecordKey to sn_protocol

## [0.4.0](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.3.2...sn_protocol-v0.4.0) - 2023-07-28

### Added
- *(protocol)* Add GetStoreCost Query and QueryResponse

### Other
- remove duplicate the thes

## [0.3.2](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.3.1...sn_protocol-v0.3.2) - 2023-07-26

### Fixed
- *(register)* Registers with same name but different tags were not being stored by the network

### Other
- centralising RecordKey creation logic to make sure we always use the same for all content type

## [0.3.1](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.3.0...sn_protocol-v0.3.1) - 2023-07-25

### Added
- *(replication)* replicate when our close group changes

## [0.3.0](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.2.10...sn_protocol-v0.3.0) - 2023-07-21

### Added
- *(node)* fee output of payment proof to be required before storing chunks
- *(protocol)* [**breaking**] make Chunks storage payment required

## [0.2.10](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.2.9...sn_protocol-v0.2.10) - 2023-07-20

### Other
- cleanup error types

## [0.2.9](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.2.8...sn_protocol-v0.2.9) - 2023-07-19

### Added
- using kad::record for dbc spend ops

## [0.2.8](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.2.7...sn_protocol-v0.2.8) - 2023-07-19

### Other
- remove un-used Query::GetRegister

## [0.2.7](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.2.6...sn_protocol-v0.2.7) - 2023-07-18

### Added
- safer registers requiring signatures

### Fixed
- address PR comments

## [0.2.6](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.2.5...sn_protocol-v0.2.6) - 2023-07-17

### Added
- *(networking)* upgrade to libp2p 0.52.0

### Other
- add missing cargo publish dry run for top level crates

## [0.2.5](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.2.4...sn_protocol-v0.2.5) - 2023-07-12

### Other
- client to upload paid chunks in batches

## [0.2.4](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.2.3...sn_protocol-v0.2.4) - 2023-07-11

### Other
- logging detailed NetworkAddress

## [0.2.3](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.2.2...sn_protocol-v0.2.3) - 2023-07-10

### Added
- client query register via get_record
- client upload Register via put_record

## [0.2.2](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.2.1...sn_protocol-v0.2.2) - 2023-07-06

### Added
- client upload chunk using kad::put_record

## [0.2.1](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.2.0...sn_protocol-v0.2.1) - 2023-07-05

### Added
- carry out validation for record_store::put

## [0.2.0](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.1.11...sn_protocol-v0.2.0) - 2023-07-05

### Added
- [**breaking**] send the list of spent dbc ids instead of whole tx within payment proof
- check fee output id when spending inputs and check paid fee amount when storing Chunks

### Other
- adapting codebase to new sn_dbc

## [0.1.11](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.1.10...sn_protocol-v0.1.11) - 2023-07-04

### Other
- demystify permissions

## [0.1.10](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.1.9...sn_protocol-v0.1.10) - 2023-06-28

### Added
- rework permissions, implement register cmd handlers
- register refactor, kad reg without cmds

### Fixed
- rename UserRights to UserPermissions

## [0.1.9](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.1.8...sn_protocol-v0.1.9) - 2023-06-21

### Added
- *(node)* trigger replication when inactivity

## [0.1.8](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.1.7...sn_protocol-v0.1.8) - 2023-06-21

### Fixed
- *(protocol)* remove unsafe indexing

### Other
- remove unused error variants
- *(node)* obtain parent_tx from SignedSpend

## [0.1.7](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.1.6...sn_protocol-v0.1.7) - 2023-06-20

### Added
- *(network)* validate `Record` on GET
- *(network)* validate and store `ReplicatedData`
- *(node)* perform proper validations on PUT
- *(network)* store `Chunk` along with `PaymentProof`
- *(kad)* impl `RecordHeader` to store the record kind

### Fixed
- *(record_header)* encode unit enum as u32
- *(node)* store parent tx along with `SignedSpend`
- *(network)* use `rmp_serde` for `RecordHeader` ser/de

### Other
- *(docs)* add more docs and comments

## [0.1.6](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.1.5...sn_protocol-v0.1.6) - 2023-06-20

### Added
- nodes to verify input DBCs of Chunk payment proof were spent

### Other
- specific error types for different payment proof verification scenarios
- include the Tx instead of output DBCs as part of storage payment proofs

## [0.1.5](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.1.4...sn_protocol-v0.1.5) - 2023-06-15

### Added
- add double spend test

### Fixed
- parent spend checks
- parent spend issue

## [0.1.4](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.1.3...sn_protocol-v0.1.4) - 2023-06-14

### Added
- include output DBC within payment proof for Chunks storage

## [0.1.3](https://github.com/maidsafe/safe_network/compare/sn_protocol-v0.1.2...sn_protocol-v0.1.3) - 2023-06-09

### Fixed
- *(replication)* prevent dropped conns during replication

### Other
- manually change crate version
- Revert "chore(release): sn_cli-v0.77.1/sn_client-v0.85.2/sn_networking-v0.1.2/sn_protocol-v0.1.2/sn_node-v0.83.1/sn_record_store-v0.1.2/sn_registers-v0.1.2"

## [0.1.1](https://github.com/jacderida/safe_network/compare/sn_protocol-v0.1.0...sn_protocol-v0.1.1) - 2023-06-06

### Added
- refactor replication flow to using pull model

## [0.1.0](https://github.com/jacderida/safe_network/releases/tag/sn_protocol-v0.1.0) - 2023-06-04

### Added
- store double spends when we detect them
- record based DBC Spends

### Fixed
- remove unused deps, fix doc comment

### Other
- bump sn_dbc version to 19 for simpler signedspend debug
- accommodate new workspace
- extract new sn_protocol crate
