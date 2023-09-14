# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.81.36](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.35...sn_cli-v0.81.36) - 2023-09-14

### Other
- *(metrics)* rename feature flag and small fixes

## [0.81.35](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.34...sn_cli-v0.81.35) - 2023-09-13

### Added
- *(register)* paying nodes for Register storage

## [0.81.34](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.33...sn_cli-v0.81.34) - 2023-09-12

### Added
- utilize stream decryptor

## [0.81.33](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.32...sn_cli-v0.81.33) - 2023-09-12

### Other
- update dependencies

## [0.81.32](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.31...sn_cli-v0.81.32) - 2023-09-12

### Other
- *(metrics)* rename network metrics and remove from default features list

## [0.81.31](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.30...sn_cli-v0.81.31) - 2023-09-12

### Added
- add tx and parent spends verification
- chunk payments using UTXOs instead of DBCs

### Other
- use updated sn_dbc

## [0.81.30](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.29...sn_cli-v0.81.30) - 2023-09-11

### Other
- update dependencies

## [0.81.29](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.28...sn_cli-v0.81.29) - 2023-09-11

### Other
- utilize stream encryptor

## [0.81.28](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.27...sn_cli-v0.81.28) - 2023-09-11

### Other
- update dependencies

## [0.81.27](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.26...sn_cli-v0.81.27) - 2023-09-08

### Added
- *(client)* repay for chunks if they cannot be validated

### Fixed
- *(client)* dont bail on failed upload before verify/repay

### Other
- *(client)* refactor to have permits at network layer
- *(refactor)* remove wallet_client args from upload flow
- *(refactor)* remove upload_chunks semaphore arg

## [0.81.26](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.25...sn_cli-v0.81.26) - 2023-09-07

### Other
- update dependencies

## [0.81.25](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.24...sn_cli-v0.81.25) - 2023-09-07

### Other
- update dependencies

## [0.81.24](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.23...sn_cli-v0.81.24) - 2023-09-07

### Other
- update dependencies

## [0.81.23](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.22...sn_cli-v0.81.23) - 2023-09-06

### Other
- update dependencies

## [0.81.22](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.21...sn_cli-v0.81.22) - 2023-09-05

### Other
- update dependencies

## [0.81.21](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.20...sn_cli-v0.81.21) - 2023-09-05

### Other
- update dependencies

## [0.81.20](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.19...sn_cli-v0.81.20) - 2023-09-05

### Other
- update dependencies

## [0.81.19](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.18...sn_cli-v0.81.19) - 2023-09-05

### Added
- *(cli)* properly init color_eyre, advise on hex parse fail

## [0.81.18](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.17...sn_cli-v0.81.18) - 2023-09-05

### Other
- update dependencies

## [0.81.17](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.16...sn_cli-v0.81.17) - 2023-09-04

### Other
- update dependencies

## [0.81.16](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.15...sn_cli-v0.81.16) - 2023-09-04

### Other
- update dependencies

## [0.81.15](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.14...sn_cli-v0.81.15) - 2023-09-04

### Other
- update dependencies

## [0.81.14](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.13...sn_cli-v0.81.14) - 2023-09-04

### Other
- update dependencies

## [0.81.13](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.12...sn_cli-v0.81.13) - 2023-09-02

### Other
- update dependencies

## [0.81.12](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.11...sn_cli-v0.81.12) - 2023-09-01

### Other
- update dependencies

## [0.81.11](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.10...sn_cli-v0.81.11) - 2023-09-01

### Other
- *(cli)* better formatting for elapsed time statements
- *(transfers)* store dbcs by ref to avoid more clones

## [0.81.10](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.9...sn_cli-v0.81.10) - 2023-09-01

### Other
- update dependencies

## [0.81.9](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.8...sn_cli-v0.81.9) - 2023-09-01

### Other
- update dependencies

## [0.81.8](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.7...sn_cli-v0.81.8) - 2023-08-31

### Added
- *(cli)* perform wallet actions without connecting to the network

### Other
- remove unused async

## [0.81.7](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.6...sn_cli-v0.81.7) - 2023-08-31

### Other
- update dependencies

## [0.81.6](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.5...sn_cli-v0.81.6) - 2023-08-31

### Added
- *(cli)* wallet cmd flag enabing to query a node's local wallet balance

### Fixed
- *(cli)* don't try to create wallet paths when checking balance

## [0.81.5](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.4...sn_cli-v0.81.5) - 2023-08-31

### Other
- update dependencies

## [0.81.4](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.3...sn_cli-v0.81.4) - 2023-08-31

### Other
- update dependencies

## [0.81.3](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.2...sn_cli-v0.81.3) - 2023-08-31

### Fixed
- correct bench download calculation

## [0.81.2](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.1...sn_cli-v0.81.2) - 2023-08-31

### Other
- update dependencies

## [0.81.1](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.81.0...sn_cli-v0.81.1) - 2023-08-31

### Added
- *(cli)* expose 'concurrency' flag
- *(cli)* increase put parallelisation

### Other
- *(client)* improve download concurrency.

## [0.81.0](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.64...sn_cli-v0.81.0) - 2023-08-30

### Added
- refactor to allow greater upload parallelisation
- one transfer per data set, mapped dbcs to content addrs
- [**breaking**] pay each chunk holder direct
- feat!(protocol): get price and pay for each chunk individually
- feat!(protocol): remove chunk merkletree to simplify payment

### Fixed
- *(tokio)* remove tokio fs

### Other
- *(deps)* bump tokio to 1.32.0
- *(client)* refactor client wallet to reduce dbc clones
- *(client)* pass around content payments map mut ref
- *(client)* reduce transferoutputs cloning

## [0.80.64](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.63...sn_cli-v0.80.64) - 2023-08-30

### Other
- update dependencies

## [0.80.63](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.62...sn_cli-v0.80.63) - 2023-08-30

### Other
- update dependencies

## [0.80.62](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.61...sn_cli-v0.80.62) - 2023-08-29

### Other
- update dependencies

## [0.80.61](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.60...sn_cli-v0.80.61) - 2023-08-25

### Other
- update dependencies

## [0.80.60](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.59...sn_cli-v0.80.60) - 2023-08-24

### Other
- *(cli)* verify bench uploads once more

## [0.80.59](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.58...sn_cli-v0.80.59) - 2023-08-24

### Other
- rust 1.72.0 fixes

## [0.80.58](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.57...sn_cli-v0.80.58) - 2023-08-24

### Other
- update dependencies

## [0.80.57](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.56...sn_cli-v0.80.57) - 2023-08-22

### Other
- update dependencies

## [0.80.56](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.55...sn_cli-v0.80.56) - 2023-08-22

### Fixed
- fixes to allow upload file works properly

## [0.80.55](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.54...sn_cli-v0.80.55) - 2023-08-21

### Other
- update dependencies

## [0.80.54](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.53...sn_cli-v0.80.54) - 2023-08-21

### Other
- update dependencies

## [0.80.53](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.52...sn_cli-v0.80.53) - 2023-08-18

### Other
- update dependencies

## [0.80.52](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.51...sn_cli-v0.80.52) - 2023-08-18

### Other
- update dependencies

## [0.80.51](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.50...sn_cli-v0.80.51) - 2023-08-17

### Other
- update dependencies

## [0.80.50](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.49...sn_cli-v0.80.50) - 2023-08-17

### Other
- update dependencies

## [0.80.49](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.48...sn_cli-v0.80.49) - 2023-08-17

### Other
- update dependencies

## [0.80.48](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.47...sn_cli-v0.80.48) - 2023-08-17

### Fixed
- avoid download bench result polluted

### Other
- more client logs

## [0.80.47](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.46...sn_cli-v0.80.47) - 2023-08-16

### Other
- update dependencies

## [0.80.46](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.45...sn_cli-v0.80.46) - 2023-08-16

### Other
- update dependencies

## [0.80.45](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.44...sn_cli-v0.80.45) - 2023-08-16

### Other
- optimize benchmark flow

## [0.80.44](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.43...sn_cli-v0.80.44) - 2023-08-15

### Other
- update dependencies

## [0.80.43](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.42...sn_cli-v0.80.43) - 2023-08-14

### Other
- update dependencies

## [0.80.42](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.41...sn_cli-v0.80.42) - 2023-08-14

### Other
- update dependencies

## [0.80.41](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.40...sn_cli-v0.80.41) - 2023-08-11

### Other
- *(cli)* print cost info

## [0.80.40](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.39...sn_cli-v0.80.40) - 2023-08-11

### Other
- update dependencies

## [0.80.39](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.38...sn_cli-v0.80.39) - 2023-08-10

### Other
- update dependencies

## [0.80.38](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.37...sn_cli-v0.80.38) - 2023-08-10

### Other
- update dependencies

## [0.80.37](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.36...sn_cli-v0.80.37) - 2023-08-09

### Other
- update dependencies

## [0.80.36](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.35...sn_cli-v0.80.36) - 2023-08-08

### Fixed
- *(cli)* remove manual faucet claim from benchmarking.
- *(node)* prevent panic in storage calcs

### Other
- *(cli)* get more money for benching
- log bench errors

## [0.80.35](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.34...sn_cli-v0.80.35) - 2023-08-07

### Other
- update dependencies

## [0.80.34](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.33...sn_cli-v0.80.34) - 2023-08-07

### Other
- *(node)* dont verify during benchmarks

## [0.80.33](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.32...sn_cli-v0.80.33) - 2023-08-07

### Added
- rework register addresses to include pk

### Other
- cleanup comments and names

## [0.80.32](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.31...sn_cli-v0.80.32) - 2023-08-07

### Other
- update dependencies

## [0.80.31](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.30...sn_cli-v0.80.31) - 2023-08-04

### Other
- update dependencies

## [0.80.30](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.29...sn_cli-v0.80.30) - 2023-08-04

### Other
- update dependencies

## [0.80.29](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.28...sn_cli-v0.80.29) - 2023-08-03

### Other
- update dependencies

## [0.80.28](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.27...sn_cli-v0.80.28) - 2023-08-03

### Other
- update dependencies

## [0.80.27](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.26...sn_cli-v0.80.27) - 2023-08-03

### Other
- update dependencies

## [0.80.26](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.25...sn_cli-v0.80.26) - 2023-08-03

### Other
- update dependencies

## [0.80.25](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.24...sn_cli-v0.80.25) - 2023-08-03

### Other
- update dependencies

## [0.80.24](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.23...sn_cli-v0.80.24) - 2023-08-02

### Other
- update dependencies

## [0.80.23](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.22...sn_cli-v0.80.23) - 2023-08-02

### Other
- update dependencies

## [0.80.22](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.21...sn_cli-v0.80.22) - 2023-08-01

### Other
- update dependencies

## [0.80.21](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.20...sn_cli-v0.80.21) - 2023-08-01

### Other
- update dependencies

## [0.80.20](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.19...sn_cli-v0.80.20) - 2023-08-01

### Other
- update dependencies

## [0.80.19](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.18...sn_cli-v0.80.19) - 2023-08-01

### Other
- update dependencies

## [0.80.18](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.17...sn_cli-v0.80.18) - 2023-08-01

### Added
- *(cli)* add no-verify flag to cli

### Other
- *(cli)* update logs and ci for payments

## [0.80.17](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.16...sn_cli-v0.80.17) - 2023-08-01

### Other
- update dependencies

## [0.80.16](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.15...sn_cli-v0.80.16) - 2023-07-31

### Other
- update dependencies

## [0.80.15](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.14...sn_cli-v0.80.15) - 2023-07-31

### Other
- update dependencies

## [0.80.14](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.13...sn_cli-v0.80.14) - 2023-07-31

### Other
- update dependencies

## [0.80.13](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.12...sn_cli-v0.80.13) - 2023-07-31

### Other
- update dependencies

## [0.80.12](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.11...sn_cli-v0.80.12) - 2023-07-28

### Other
- update dependencies

## [0.80.11](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.10...sn_cli-v0.80.11) - 2023-07-28

### Other
- update dependencies

## [0.80.10](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.9...sn_cli-v0.80.10) - 2023-07-28

### Other
- update dependencies

## [0.80.9](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.8...sn_cli-v0.80.9) - 2023-07-28

### Other
- update dependencies

## [0.80.8](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.7...sn_cli-v0.80.8) - 2023-07-27

### Other
- update dependencies

## [0.80.7](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.6...sn_cli-v0.80.7) - 2023-07-26

### Other
- update dependencies

## [0.80.6](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.5...sn_cli-v0.80.6) - 2023-07-26

### Other
- update dependencies

## [0.80.5](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.4...sn_cli-v0.80.5) - 2023-07-26

### Other
- update dependencies

## [0.80.4](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.3...sn_cli-v0.80.4) - 2023-07-26

### Other
- update dependencies

## [0.80.3](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.2...sn_cli-v0.80.3) - 2023-07-26

### Other
- update dependencies

## [0.80.2](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.1...sn_cli-v0.80.2) - 2023-07-26

### Other
- update dependencies

## [0.80.1](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.80.0...sn_cli-v0.80.1) - 2023-07-25

### Other
- update dependencies

## [0.80.0](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.32...sn_cli-v0.80.0) - 2023-07-21

### Added
- *(cli)* allow to pass the hex-encoded DBC as arg
- *(protocol)* [**breaking**] make Chunks storage payment required

## [0.79.32](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.31...sn_cli-v0.79.32) - 2023-07-20

### Other
- update dependencies

## [0.79.31](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.30...sn_cli-v0.79.31) - 2023-07-20

### Other
- update dependencies

## [0.79.30](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.29...sn_cli-v0.79.30) - 2023-07-19

### Other
- update dependencies

## [0.79.29](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.28...sn_cli-v0.79.29) - 2023-07-19

### Other
- update dependencies

## [0.79.28](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.27...sn_cli-v0.79.28) - 2023-07-19

### Other
- update dependencies

## [0.79.27](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.26...sn_cli-v0.79.27) - 2023-07-19

### Other
- update dependencies

## [0.79.26](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.25...sn_cli-v0.79.26) - 2023-07-18

### Other
- update dependencies

## [0.79.25](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.24...sn_cli-v0.79.25) - 2023-07-18

### Other
- update dependencies

## [0.79.24](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.23...sn_cli-v0.79.24) - 2023-07-18

### Fixed
- client

## [0.79.23](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.22...sn_cli-v0.79.23) - 2023-07-18

### Other
- update dependencies

## [0.79.22](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.21...sn_cli-v0.79.22) - 2023-07-17

### Fixed
- *(cli)* add more context when failing to decode a wallet

## [0.79.21](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.20...sn_cli-v0.79.21) - 2023-07-17

### Other
- update dependencies

## [0.79.20](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.19...sn_cli-v0.79.20) - 2023-07-17

### Added
- *(networking)* upgrade to libp2p 0.52.0

## [0.79.19](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.18...sn_cli-v0.79.19) - 2023-07-17

### Added
- *(client)* keep storage payment proofs in local wallet

## [0.79.18](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.17...sn_cli-v0.79.18) - 2023-07-13

### Other
- update dependencies

## [0.79.17](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.16...sn_cli-v0.79.17) - 2023-07-13

### Other
- update dependencies

## [0.79.16](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.15...sn_cli-v0.79.16) - 2023-07-12

### Other
- client to upload paid chunks in batches
- chunk files only once when making payment for their storage

## [0.79.15](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.14...sn_cli-v0.79.15) - 2023-07-11

### Other
- update dependencies

## [0.79.14](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.13...sn_cli-v0.79.14) - 2023-07-11

### Fixed
- *(client)* publish register on creation

## [0.79.13](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.12...sn_cli-v0.79.13) - 2023-07-11

### Other
- update dependencies

## [0.79.12](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.11...sn_cli-v0.79.12) - 2023-07-11

### Other
- update dependencies

## [0.79.11](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.10...sn_cli-v0.79.11) - 2023-07-11

### Other
- update dependencies

## [0.79.10](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.9...sn_cli-v0.79.10) - 2023-07-10

### Other
- update dependencies

## [0.79.9](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.8...sn_cli-v0.79.9) - 2023-07-10

### Other
- update dependencies

## [0.79.8](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.7...sn_cli-v0.79.8) - 2023-07-10

### Added
- faucet server and cli DBC read

### Fixed
- use Deposit --stdin instead of Read in cli
- wallet store

## [0.79.7](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.6...sn_cli-v0.79.7) - 2023-07-10

### Other
- update dependencies

## [0.79.6](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.5...sn_cli-v0.79.6) - 2023-07-07

### Other
- update dependencies

## [0.79.5](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.4...sn_cli-v0.79.5) - 2023-07-07

### Other
- update dependencies

## [0.79.4](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.3...sn_cli-v0.79.4) - 2023-07-07

### Other
- update dependencies

## [0.79.3](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.2...sn_cli-v0.79.3) - 2023-07-07

### Other
- update dependencies

## [0.79.2](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.1...sn_cli-v0.79.2) - 2023-07-06

### Other
- update dependencies

## [0.79.1](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.79.0...sn_cli-v0.79.1) - 2023-07-06

### Other
- update dependencies

## [0.79.0](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.26...sn_cli-v0.79.0) - 2023-07-06

### Added
- introduce `--log-format` arguments
- provide `--log-output-dest` arg for `safe`
- provide `--log-output-dest` arg for `safenode`

### Other
- use data-dir rather than root-dir
- incorporate various feedback items

## [0.78.26](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.25...sn_cli-v0.78.26) - 2023-07-05

### Other
- update dependencies

## [0.78.25](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.24...sn_cli-v0.78.25) - 2023-07-05

### Other
- update dependencies

## [0.78.24](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.23...sn_cli-v0.78.24) - 2023-07-05

### Other
- update dependencies

## [0.78.23](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.22...sn_cli-v0.78.23) - 2023-07-04

### Other
- update dependencies

## [0.78.22](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.21...sn_cli-v0.78.22) - 2023-07-03

### Other
- reduce SAMPLE_SIZE for the data_with_churn test

## [0.78.21](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.20...sn_cli-v0.78.21) - 2023-06-29

### Other
- update dependencies

## [0.78.20](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.19...sn_cli-v0.78.20) - 2023-06-29

### Other
- update dependencies

## [0.78.19](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.18...sn_cli-v0.78.19) - 2023-06-28

### Other
- update dependencies

## [0.78.18](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.17...sn_cli-v0.78.18) - 2023-06-28

### Added
- register refactor, kad reg without cmds

## [0.78.17](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.16...sn_cli-v0.78.17) - 2023-06-28

### Other
- update dependencies

## [0.78.16](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.15...sn_cli-v0.78.16) - 2023-06-28

### Other
- update dependencies

## [0.78.15](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.14...sn_cli-v0.78.15) - 2023-06-27

### Other
- update dependencies

## [0.78.14](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.13...sn_cli-v0.78.14) - 2023-06-27

### Other
- update dependencies

## [0.78.13](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.12...sn_cli-v0.78.13) - 2023-06-27

### Other
- benchmark client download

## [0.78.12](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.11...sn_cli-v0.78.12) - 2023-06-26

### Other
- update dependencies

## [0.78.11](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.10...sn_cli-v0.78.11) - 2023-06-26

### Other
- update dependencies

## [0.78.10](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.9...sn_cli-v0.78.10) - 2023-06-26

### Other
- update dependencies

## [0.78.9](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.8...sn_cli-v0.78.9) - 2023-06-26

### Other
- update dependencies

## [0.78.8](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.7...sn_cli-v0.78.8) - 2023-06-26

### Other
- update dependencies

## [0.78.7](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.6...sn_cli-v0.78.7) - 2023-06-24

### Other
- update dependencies

## [0.78.6](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.5...sn_cli-v0.78.6) - 2023-06-23

### Other
- update dependencies

## [0.78.5](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.4...sn_cli-v0.78.5) - 2023-06-23

### Other
- update dependencies

## [0.78.4](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.3...sn_cli-v0.78.4) - 2023-06-23

### Other
- update dependencies

## [0.78.3](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.2...sn_cli-v0.78.3) - 2023-06-23

### Other
- update dependencies

## [0.78.2](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.1...sn_cli-v0.78.2) - 2023-06-22

### Other
- update dependencies

## [0.78.1](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.78.0...sn_cli-v0.78.1) - 2023-06-22

### Other
- *(client)* initial refactor around uploads

## [0.78.0](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.49...sn_cli-v0.78.0) - 2023-06-22

### Added
- use standarised directories for files/wallet commands

## [0.77.49](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.48...sn_cli-v0.77.49) - 2023-06-21

### Other
- update dependencies

## [0.77.48](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.47...sn_cli-v0.77.48) - 2023-06-21

### Other
- update dependencies

## [0.77.47](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.46...sn_cli-v0.77.47) - 2023-06-21

### Other
- *(node)* obtain parent_tx from SignedSpend

## [0.77.46](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.45...sn_cli-v0.77.46) - 2023-06-21

### Added
- provide option for log output in json

## [0.77.45](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.44...sn_cli-v0.77.45) - 2023-06-20

### Other
- update dependencies

## [0.77.44](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.43...sn_cli-v0.77.44) - 2023-06-20

### Other
- update dependencies

## [0.77.43](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.42...sn_cli-v0.77.43) - 2023-06-20

### Other
- include the Tx instead of output DBCs as part of storage payment proofs
- use a set to collect Chunks addrs for build payment proof

## [0.77.42](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.41...sn_cli-v0.77.42) - 2023-06-20

### Other
- update dependencies

## [0.77.41](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.40...sn_cli-v0.77.41) - 2023-06-20

### Other
- update dependencies

## [0.77.40](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.39...sn_cli-v0.77.40) - 2023-06-20

### Other
- update dependencies

## [0.77.39](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.38...sn_cli-v0.77.39) - 2023-06-20

### Other
- update dependencies

## [0.77.38](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.37...sn_cli-v0.77.38) - 2023-06-20

### Other
- update dependencies

## [0.77.37](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.36...sn_cli-v0.77.37) - 2023-06-19

### Other
- update dependencies

## [0.77.36](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.35...sn_cli-v0.77.36) - 2023-06-19

### Other
- update dependencies

## [0.77.35](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.34...sn_cli-v0.77.35) - 2023-06-19

### Other
- update dependencies

## [0.77.34](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.33...sn_cli-v0.77.34) - 2023-06-19

### Other
- update dependencies

## [0.77.33](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.32...sn_cli-v0.77.33) - 2023-06-19

### Other
- update dependencies

## [0.77.32](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.31...sn_cli-v0.77.32) - 2023-06-19

### Fixed
- *(safe)* check if upload path contains a file

## [0.77.31](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.30...sn_cli-v0.77.31) - 2023-06-16

### Fixed
- CLI is missing local-discovery feature

## [0.77.30](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.29...sn_cli-v0.77.30) - 2023-06-16

### Other
- update dependencies

## [0.77.29](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.28...sn_cli-v0.77.29) - 2023-06-16

### Other
- update dependencies

## [0.77.28](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.27...sn_cli-v0.77.28) - 2023-06-16

### Other
- improve memory benchmarks, remove broken download bench

## [0.77.27](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.26...sn_cli-v0.77.27) - 2023-06-16

### Other
- update dependencies

## [0.77.26](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.25...sn_cli-v0.77.26) - 2023-06-16

### Fixed
- *(bin)* negate local-discovery check

## [0.77.25](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.24...sn_cli-v0.77.25) - 2023-06-16

### Other
- update dependencies

## [0.77.24](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.23...sn_cli-v0.77.24) - 2023-06-15

### Other
- update dependencies

## [0.77.23](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.22...sn_cli-v0.77.23) - 2023-06-15

### Fixed
- parent spend issue

## [0.77.22](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.21...sn_cli-v0.77.22) - 2023-06-15

### Other
- update dependencies

## [0.77.21](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.20...sn_cli-v0.77.21) - 2023-06-15

### Other
- update dependencies

## [0.77.20](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.19...sn_cli-v0.77.20) - 2023-06-15

### Other
- update dependencies

## [0.77.19](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.18...sn_cli-v0.77.19) - 2023-06-15

### Other
- use throughput for benchmarking

## [0.77.18](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.17...sn_cli-v0.77.18) - 2023-06-15

### Other
- add initial benchmarks for prs and chart generation

## [0.77.17](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.16...sn_cli-v0.77.17) - 2023-06-14

### Added
- include output DBC within payment proof for Chunks storage

## [0.77.16](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.15...sn_cli-v0.77.16) - 2023-06-14

### Other
- update dependencies

## [0.77.15](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.14...sn_cli-v0.77.15) - 2023-06-14

### Other
- use clap env and parse multiaddr

## [0.77.14](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.13...sn_cli-v0.77.14) - 2023-06-14

### Added
- *(client)* expose req/resp timeout to client cli

### Other
- *(client)* parse duration in clap derivation

## [0.77.13](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.12...sn_cli-v0.77.13) - 2023-06-13

### Other
- update dependencies

## [0.77.12](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.11...sn_cli-v0.77.12) - 2023-06-13

### Other
- update dependencies

## [0.77.11](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.10...sn_cli-v0.77.11) - 2023-06-12

### Other
- update dependencies

## [0.77.10](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.9...sn_cli-v0.77.10) - 2023-06-12

### Other
- update dependencies

## [0.77.9](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.8...sn_cli-v0.77.9) - 2023-06-09

### Other
- improve documentation for cli commands

## [0.77.8](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.7...sn_cli-v0.77.8) - 2023-06-09

### Other
- manually change crate version

## [0.77.7](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.6...sn_cli-v0.77.7) - 2023-06-09

### Other
- update dependencies

## [0.77.6](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.5...sn_cli-v0.77.6) - 2023-06-09

### Other
- emit git info with vergen

## [0.77.5](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.4...sn_cli-v0.77.5) - 2023-06-09

### Other
- update dependencies

## [0.77.4](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.3...sn_cli-v0.77.4) - 2023-06-09

### Other
- provide clarity on command arguments

## [0.77.3](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.2...sn_cli-v0.77.3) - 2023-06-08

### Other
- update dependencies

## [0.77.2](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.1...sn_cli-v0.77.2) - 2023-06-08

### Other
- improve documentation for cli arguments

## [0.77.1](https://github.com/maidsafe/safe_network/compare/sn_cli-v0.77.0...sn_cli-v0.77.1) - 2023-06-07

### Added
- making the CLI --peer arg global so it can be passed in any order
- bail out if empty list of addreses is provided for payment proof generation
- *(client)* add progress indicator for initial network connections
- attach payment proof when uploading Chunks
- collect payment proofs and make sure merkletree always has pow-of-2 leaves
- node side payment proof validation from a given Chunk, audit trail, and reason-hash
- use all Chunks of a file to generate payment the payment proof tree
- Chunk storage payment and building payment proofs

### Other
- Revert "chore(release): sn_cli-v0.77.1/sn_client-v0.85.2/sn_networking-v0.1.2/sn_node-v0.83.1"
- improve CLI --peer arg doc
- *(release)* sn_cli-v0.77.1/sn_client-v0.85.2/sn_networking-v0.1.2/sn_node-v0.83.1
- Revert "chore(release): sn_cli-v0.77.1/sn_client-v0.85.2/sn_networking-v0.1.2/sn_protocol-v0.1.2/sn_node-v0.83.1/sn_record_store-v0.1.2/sn_registers-v0.1.2"
- *(release)* sn_cli-v0.77.1/sn_client-v0.85.2/sn_networking-v0.1.2/sn_protocol-v0.1.2/sn_node-v0.83.1/sn_record_store-v0.1.2/sn_registers-v0.1.2
- *(logs)* enable metrics feature by default
- small log wording updates
- making Chunk payment proof optional for now
- moving all payment proofs utilities into sn_transfers crate
