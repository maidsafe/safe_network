# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.104.9](https://github.com/joshuef/safe_network/compare/alpha-sn_client-0.104.8...alpha-sn_client-0.104.9) - 2024-02-13

### Other
- another change

## [0.104.8](https://github.com/maidsafe/safe_network/compare/sn_client-v0.104.7...sn_client-v0.104.8) - 2024-02-12

### Other
- updated the following local packages: sn_networking

## [0.104.7](https://github.com/maidsafe/safe_network/compare/sn_client-v0.104.6...sn_client-v0.104.7) - 2024-02-12

### Other
- updated the following local packages: sn_networking

## [0.104.6](https://github.com/maidsafe/safe_network/compare/sn_client-v0.104.5...sn_client-v0.104.6) - 2024-02-12

### Added
- *(cli)* single payment for all folders being synced
- *(cli)* adding Folders download CLI cmd
- *(client)* adding Folders sync API and CLI cmd

### Other
- *(cli)* improvements based on peer review

## [0.104.5](https://github.com/maidsafe/safe_network/compare/sn_client-v0.104.4...sn_client-v0.104.5) - 2024-02-09

### Other
- updated the following local packages: sn_networking

## [0.104.4](https://github.com/maidsafe/safe_network/compare/sn_client-v0.104.3...sn_client-v0.104.4) - 2024-02-09

### Other
- updated the following local packages: sn_networking

## [0.104.3](https://github.com/maidsafe/safe_network/compare/sn_client-v0.104.2...sn_client-v0.104.3) - 2024-02-08

### Other
- copyright update to current year

## [0.104.2](https://github.com/maidsafe/safe_network/compare/sn_client-v0.104.1...sn_client-v0.104.2) - 2024-02-08

### Added
- move the RetryStrategy into protocol and use that during cli upload/download
- *(client)* perform more retries if we are verifying a register
- *(network)* impl RetryStrategy to make the reattempts flexible

### Fixed
- *(ci)* update the reattempt flag to retry_strategy flag for the cli

### Other
- *(network)* rename re-attempts to retry strategy

## [0.104.1](https://github.com/maidsafe/safe_network/compare/sn_client-v0.104.0...sn_client-v0.104.1) - 2024-02-08

### Other
- updated the following local packages: sn_networking

## [0.104.0](https://github.com/maidsafe/safe_network/compare/sn_client-v0.103.7...sn_client-v0.104.0) - 2024-02-07

### Added
- *(client)* put register to the peer that we paid to
- *(client)* [**breaking**] make the result of the storage payment into a struct

### Fixed
- rust docs error

## [0.103.7](https://github.com/maidsafe/safe_network/compare/sn_client-v0.103.6...sn_client-v0.103.7) - 2024-02-07

### Added
- extendable local state DAG in cli

## [0.103.6](https://github.com/maidsafe/safe_network/compare/sn_client-v0.103.5...sn_client-v0.103.6) - 2024-02-06

### Other
- updated the following local packages: sn_transfers

## [0.103.5](https://github.com/maidsafe/safe_network/compare/sn_client-v0.103.4...sn_client-v0.103.5) - 2024-02-05

### Other
- updated the following local packages: sn_networking

## [0.103.4](https://github.com/maidsafe/safe_network/compare/sn_client-v0.103.3...sn_client-v0.103.4) - 2024-02-05

### Other
- updated the following local packages: sn_networking

## [0.103.3](https://github.com/maidsafe/safe_network/compare/sn_client-v0.103.2...sn_client-v0.103.3) - 2024-02-05

### Other
- change to hot wallet
- docs formatting
- cargo fmt changes
- example for api verify uploaded chunks
- example for api verify cash note redemptions
- example for api publish on topic
- example for api unsubscribe to topic
- example for api subscribe to topic
- example for api get spend from network
- example for api verify register stored
- example for api get chunk
- example for api store chunk
- example for api create and pay for register
- example for api get register
- example for api get signed reg from network
- example for api signer pk
- example for api signer
- example for api sign
- example for api events channel
- example for api new
- apply format and params to doc templates
- better template set
- mark applicable functions with empty headers

## [0.103.2](https://github.com/maidsafe/safe_network/compare/sn_client-v0.103.1...sn_client-v0.103.2) - 2024-02-05

### Other
- updated the following local packages: sn_protocol

## [0.103.1](https://github.com/maidsafe/safe_network/compare/sn_client-v0.103.0...sn_client-v0.103.1) - 2024-02-02

### Other
- updated the following local packages: sn_networking

## [0.103.0](https://github.com/maidsafe/safe_network/compare/sn_client-v0.102.22...sn_client-v0.103.0) - 2024-02-02

### Other
- [**breaking**] renaming LocalWallet to HotWallet as it holds the secret key for signing tx

## [0.102.22](https://github.com/maidsafe/safe_network/compare/sn_client-v0.102.21...sn_client-v0.102.22) - 2024-02-01

### Other
- updated the following local packages: sn_networking

## [0.102.21](https://github.com/maidsafe/safe_network/compare/sn_client-v0.102.20...sn_client-v0.102.21) - 2024-02-01

### Fixed
- *(client)* error out when fetching large data_map

## [0.102.20](https://github.com/maidsafe/safe_network/compare/sn_client-v0.102.19...sn_client-v0.102.20) - 2024-02-01

### Other
- updated the following local packages: sn_networking

## [0.102.19](https://github.com/maidsafe/safe_network/compare/sn_client-v0.102.18...sn_client-v0.102.19) - 2024-01-31

### Other
- nano tokens to network address
- change to question mark from expect
- test doc changes to remove code and refactor for pr
- broadcast signed spends
- send
- verify cash note
- receive and cargo fmt
- send spends

## [0.102.18](https://github.com/maidsafe/safe_network/compare/sn_client-v0.102.17...sn_client-v0.102.18) - 2024-01-31

### Other
- updated the following local packages: sn_networking, sn_protocol

## [0.102.17](https://github.com/maidsafe/safe_network/compare/sn_client-v0.102.16...sn_client-v0.102.17) - 2024-01-30

### Other
- *(client)* log client upload failure error

## [0.102.16](https://github.com/maidsafe/safe_network/compare/sn_client-v0.102.15...sn_client-v0.102.16) - 2024-01-30

### Fixed
- *(client)* error out on verify_chunk_store

## [0.102.15](https://github.com/maidsafe/safe_network/compare/sn_client-v0.102.14...sn_client-v0.102.15) - 2024-01-30

### Other
- updated the following local packages: sn_networking

## [0.102.14](https://github.com/maidsafe/safe_network/compare/sn_client-v0.102.13...sn_client-v0.102.14) - 2024-01-30

### Other
- updated the following local packages: sn_protocol

## [0.102.13](https://github.com/maidsafe/safe_network/compare/sn_client-v0.102.12...sn_client-v0.102.13) - 2024-01-29

### Other
- *(sn_transfers)* making some functions/helpers to be constructor methods of public structs

## [0.102.12](https://github.com/maidsafe/safe_network/compare/sn_client-v0.102.11...sn_client-v0.102.12) - 2024-01-25

### Other
- improved pay for storage
- mut wallet description
- revert to mut wallet
- change to wallet result
- cargo fmt
- into wallet doc
- into wallet doc
- expand abbreviations mutable wallet
- pay for storage clone for test pass
- expand on abbreviation and added detail
- pay for records example
- pay for records and cleanup
- pay for storage once detail
- send unsigned detail
- pay for storage
- get store cost at addr unused

## [0.102.11](https://github.com/maidsafe/safe_network/compare/sn_client-v0.102.10...sn_client-v0.102.11) - 2024-01-25

### Other
- updated the following local packages: sn_networking

## [0.102.10](https://github.com/maidsafe/safe_network/compare/sn_client-v0.102.9...sn_client-v0.102.10) - 2024-01-25

### Added
- client webtransport-websys feat

### Other
- use a single target_arch.rs to simplify imports for wasm32 or no

## [0.102.9](https://github.com/maidsafe/safe_network/compare/sn_client-v0.102.8...sn_client-v0.102.9) - 2024-01-24

### Other
- updated the following local packages: sn_networking, sn_networking

## [0.102.8](https://github.com/maidsafe/safe_network/compare/sn_client-v0.102.7...sn_client-v0.102.8) - 2024-01-24

### Added
- client webtransport-websys feat

### Other
- tidy up wasm32 as target arch rather than a feat

## [0.102.7](https://github.com/maidsafe/safe_network/compare/sn_client-v0.102.6...sn_client-v0.102.7) - 2024-01-23

### Other
- *(release)* sn_protocol-v0.10.14/sn_networking-v0.12.35

## [0.102.6](https://github.com/maidsafe/safe_network/compare/sn_client-v0.102.5...sn_client-v0.102.6) - 2024-01-22

### Other
- wallet docs

## [0.102.5](https://github.com/maidsafe/safe_network/compare/sn_client-v0.102.4...sn_client-v0.102.5) - 2024-01-22

### Added
- spend dag utils

## [0.102.4](https://github.com/maidsafe/safe_network/compare/sn_client-v0.102.3...sn_client-v0.102.4) - 2024-01-18

### Other
- updated the following local packages: sn_protocol

## [0.102.3](https://github.com/maidsafe/safe_network/compare/sn_client-v0.102.2...sn_client-v0.102.3) - 2024-01-18

### Added
- set quic as default transport

## [0.102.2](https://github.com/maidsafe/safe_network/compare/sn_client-v0.102.1...sn_client-v0.102.2) - 2024-01-18

### Other
- updated the following local packages: sn_transfers

## [0.102.1](https://github.com/maidsafe/safe_network/compare/sn_client-v0.102.0...sn_client-v0.102.1) - 2024-01-17

### Other
- fixed typo
- filled missing arguments
- formatting
- formatting
- new wallet docs

## [0.102.0](https://github.com/maidsafe/safe_network/compare/sn_client-v0.101.13...sn_client-v0.102.0) - 2024-01-17

### Fixed
- *(docs)* update Client signature for doc test
- *(client)* move out the peers added var to event handler loop

### Other
- *(client)* [**breaking**] move out client connection progress bar

## [0.101.13](https://github.com/maidsafe/safe_network/compare/sn_client-v0.101.12...sn_client-v0.101.13) - 2024-01-17

### Other
- new wallet client example

## [0.101.12](https://github.com/maidsafe/safe_network/compare/sn_client-v0.101.11...sn_client-v0.101.12) - 2024-01-16

### Other
- updated the following local packages: sn_transfers

## [0.101.11](https://github.com/maidsafe/safe_network/compare/sn_client-v0.101.10...sn_client-v0.101.11) - 2024-01-15

### Fixed
- *(client)* avoid deadlock during upload in case of error

## [0.101.10](https://github.com/maidsafe/safe_network/compare/sn_client-v0.101.9...sn_client-v0.101.10) - 2024-01-15

### Other
- updated the following local packages: sn_protocol

## [0.101.9](https://github.com/maidsafe/safe_network/compare/sn_client-v0.101.8...sn_client-v0.101.9) - 2024-01-15

### Fixed
- *(client)* cache payments via disk instead of memory map

### Other
- *(client)* collect wallet handling time statistics

## [0.101.8](https://github.com/maidsafe/safe_network/compare/sn_client-v0.101.7...sn_client-v0.101.8) - 2024-01-12

### Other
- updated the following local packages: sn_networking

## [0.101.7](https://github.com/maidsafe/safe_network/compare/sn_client-v0.101.6...sn_client-v0.101.7) - 2024-01-12

### Fixed
- *(client)* avoid dead lock with less chunks

## [0.101.6](https://github.com/maidsafe/safe_network/compare/sn_client-v0.101.5...sn_client-v0.101.6) - 2024-01-11

### Other
- *(client)* refactor client upload flow

## [0.101.5](https://github.com/maidsafe/safe_network/compare/sn_client-v0.101.4...sn_client-v0.101.5) - 2024-01-11

### Added
- error if file size smaller than MIN_ENCRYPTABLE_BYTES

### Other
- udpate self_encryption dep

## [0.101.4](https://github.com/maidsafe/safe_network/compare/sn_client-v0.101.3...sn_client-v0.101.4) - 2024-01-11

### Other
- updated the following local packages: sn_networking

## [0.101.3](https://github.com/maidsafe/safe_network/compare/sn_client-v0.101.2...sn_client-v0.101.3) - 2024-01-10

### Added
- *(client)* client APIs and CLI cmd to broadcast a transaction signed offline

### Other
- fixup send_spends and use ExcessiveNanoValue error

## [0.101.2](https://github.com/maidsafe/safe_network/compare/sn_client-v0.101.1...sn_client-v0.101.2) - 2024-01-10

### Added
- allow register CLI to create a public register writable to anyone

## [0.101.1](https://github.com/maidsafe/safe_network/compare/sn_client-v0.101.0...sn_client-v0.101.1) - 2024-01-09

### Other
- updated the following local packages: sn_networking, sn_transfers

## [0.101.0](https://github.com/maidsafe/safe_network/compare/sn_client-v0.100.1...sn_client-v0.101.0) - 2024-01-09

### Added
- *(client)* use buffered future stream to download chunks

### Fixed
- *(client)* empty out the download cache once the stream exits
- *(ci)* fix clippy error due to Send not being general

### Other
- *(client)* add docs to FilesDownload
- *(client)* [**breaking**] move read_from range into `DownloadFiles`

## [0.100.1](https://github.com/maidsafe/safe_network/compare/sn_client-v0.100.0...sn_client-v0.100.1) - 2024-01-09

### Other
- get spend from network only require Majority

## [0.100.0](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.42...sn_client-v0.100.0) - 2024-01-08

### Added
- *(cli)* intergrate FilesDownload with cli
- *(client)* emit events from download process

### Other
- *(client)* [**breaking**] refactor `Files` into `FilesUpload`

## [0.99.42](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.41...sn_client-v0.99.42) - 2024-01-08

### Other
- updated the following local packages: sn_networking

## [0.99.41](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.40...sn_client-v0.99.41) - 2024-01-08

### Other
- more doc updates to readme files

## [0.99.40](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.39...sn_client-v0.99.40) - 2024-01-08

### Fixed
- *(client)* reset sequential_payment_fails on batch upload success

## [0.99.39](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.38...sn_client-v0.99.39) - 2024-01-05

### Other
- add clippy unwrap lint to workspace

## [0.99.38](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.37...sn_client-v0.99.38) - 2024-01-05

### Added
- *(network)* move the kad::put_record_to inside PutRecordCfg

## [0.99.37](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.36...sn_client-v0.99.37) - 2024-01-03

### Added
- *(client)* clients no longer upload data_map by default

### Other
- refactor for clarity around head_chunk_address
- *(cli)* do not write datamap chunk if non-public

## [0.99.36](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.35...sn_client-v0.99.36) - 2024-01-03

### Other
- updated the following local packages: sn_networking

## [0.99.35](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.34...sn_client-v0.99.35) - 2024-01-02

### Fixed
- *(client)* wallet not progress with unconfirmed tx

## [0.99.34](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.33...sn_client-v0.99.34) - 2024-01-02

### Other
- updated the following local packages: sn_networking

## [0.99.33](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.32...sn_client-v0.99.33) - 2023-12-29

### Other
- updated the following local packages: sn_networking

## [0.99.32](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.31...sn_client-v0.99.32) - 2023-12-29

### Added
- use put_record_to during upload chunk

## [0.99.31](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.30...sn_client-v0.99.31) - 2023-12-26

### Other
- updated the following local packages: sn_networking

## [0.99.30](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.29...sn_client-v0.99.30) - 2023-12-22

### Other
- updated the following local packages: sn_networking

## [0.99.29](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.28...sn_client-v0.99.29) - 2023-12-21

### Other
- *(client)* emit chunk Uploaded event if a chunk was verified during repayment

## [0.99.28](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.27...sn_client-v0.99.28) - 2023-12-20

### Other
- reduce default batch size

## [0.99.27](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.26...sn_client-v0.99.27) - 2023-12-19

### Added
- network royalties through audit POC

## [0.99.26](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.25...sn_client-v0.99.26) - 2023-12-19

### Other
- updated the following local packages: sn_networking

## [0.99.25](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.24...sn_client-v0.99.25) - 2023-12-19

### Fixed
- *(test)* tests should try to load just the faucet wallet

## [0.99.24](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.23...sn_client-v0.99.24) - 2023-12-19

### Other
- updated the following local packages: sn_networking

## [0.99.23](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.22...sn_client-v0.99.23) - 2023-12-19

### Fixed
- *(cli)* mark chunk completion as soon as we upload each chunk

## [0.99.22](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.21...sn_client-v0.99.22) - 2023-12-18

### Added
- *(transfers)* add api for cleaning up CashNotes

## [0.99.21](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.20...sn_client-v0.99.21) - 2023-12-18

### Added
- *(client)* update the Files config via setters
- *(client)* track the upload stats inside Files
- *(client)* move upload retry logic from CLI to client

### Fixed
- *(test)* use the Files struct to upload chunks

### Other
- *(client)* add docs to the Files struct

## [0.99.20](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.19...sn_client-v0.99.20) - 2023-12-14

### Other
- updated the following local packages: sn_networking, sn_protocol, sn_registers, sn_transfers

## [0.99.19](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.18...sn_client-v0.99.19) - 2023-12-14

### Added
- *(client)* add backoff to payment retries
- *(networking)* use backoff for get_record

## [0.99.18](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.17...sn_client-v0.99.18) - 2023-12-14

### Other
- *(test)* fix log messages during churn test

## [0.99.17](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.16...sn_client-v0.99.17) - 2023-12-14

### Added
- *(cli)* simple retry mechanism for remaining chunks

## [0.99.16](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.15...sn_client-v0.99.16) - 2023-12-13

### Other
- updated the following local packages: sn_networking

## [0.99.15](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.14...sn_client-v0.99.15) - 2023-12-13

### Added
- add amounts to edges
- audit DAG collection and visualization
- cli double spends audit from genesis

### Fixed
- docs

### Other
- udeps and gitignore

## [0.99.14](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.13...sn_client-v0.99.14) - 2023-12-12

### Other
- updated the following local packages: sn_protocol

## [0.99.13](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.12...sn_client-v0.99.13) - 2023-12-12

### Added
- *(cli)* skip payment and upload for existing chunks

## [0.99.12](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.11...sn_client-v0.99.12) - 2023-12-12

### Added
- constant uploading across batches

## [0.99.11](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.10...sn_client-v0.99.11) - 2023-12-11

### Other
- updated the following local packages: sn_networking

## [0.99.10](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.9...sn_client-v0.99.10) - 2023-12-07

### Other
- updated the following local packages: sn_networking

## [0.99.9](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.8...sn_client-v0.99.9) - 2023-12-06

### Other
- *(network)* use PUT Quorum::One for chunks
- *(network)* add more docs to the get_record_handlers

## [0.99.8](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.7...sn_client-v0.99.8) - 2023-12-06

### Other
- updated the following local packages: sn_networking

## [0.99.7](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.6...sn_client-v0.99.7) - 2023-12-06

### Other
- updated the following local packages: sn_transfers

## [0.99.6](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.5...sn_client-v0.99.6) - 2023-12-06

### Other
- remove some needless cloning
- remove needless pass by value
- use inline format args
- add boilerplate for workspace lints

## [0.99.5](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.4...sn_client-v0.99.5) - 2023-12-05

### Added
- *(network)* use custom enum for get_record errors

### Other
- *(network)* avoid losing error info by converting them to a single type

## [0.99.4](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.3...sn_client-v0.99.4) - 2023-12-05

### Other
- updated the following local packages: sn_transfers

## [0.99.3](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.2...sn_client-v0.99.3) - 2023-12-05

### Other
- updated the following local packages: sn_networking

## [0.99.2](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.1...sn_client-v0.99.2) - 2023-12-05

### Added
- allow for cli chunk put retries for un verifiable chunks

### Fixed
- mark chunks as completed when no failures on retry

## [0.99.1](https://github.com/maidsafe/safe_network/compare/sn_client-v0.99.0...sn_client-v0.99.1) - 2023-12-05

### Fixed
- *(client)* dont assume verification is always set w/ VerificationConfig

### Other
- tie node reward test to number of data.
- *(networking)* remove triggered bootstrap slowdown

## [0.99.0](https://github.com/maidsafe/safe_network/compare/sn_client-v0.98.23...sn_client-v0.99.0) - 2023-12-01

### Added
- *(network)* use seperate PUT/GET configs

### Other
- *(ci)* fix CI build cache parsing error
- *(network)* [**breaking**] use the Quorum struct provided by libp2p

## [0.98.23](https://github.com/maidsafe/safe_network/compare/sn_client-v0.98.22...sn_client-v0.98.23) - 2023-11-29

### Other
- updated the following local packages: sn_networking

## [0.98.22](https://github.com/maidsafe/safe_network/compare/sn_client-v0.98.21...sn_client-v0.98.22) - 2023-11-29

### Other
- updated the following local packages: sn_networking

## [0.98.21](https://github.com/maidsafe/safe_network/compare/sn_client-v0.98.20...sn_client-v0.98.21) - 2023-11-29

### Added
- add missing quic features

## [0.98.20](https://github.com/maidsafe/safe_network/compare/sn_client-v0.98.19...sn_client-v0.98.20) - 2023-11-29

### Added
- verify all the way to genesis
- verify spends through the cli

### Fixed
- genesis check security flaw

## [0.98.19](https://github.com/maidsafe/safe_network/compare/sn_client-v0.98.18...sn_client-v0.98.19) - 2023-11-28

### Added
- *(chunks)* serialise Chunks with MsgPack instead of bincode

## [0.98.18](https://github.com/maidsafe/safe_network/compare/sn_client-v0.98.17...sn_client-v0.98.18) - 2023-11-28

### Other
- updated the following local packages: sn_protocol

## [0.98.17](https://github.com/maidsafe/safe_network/compare/sn_client-v0.98.16...sn_client-v0.98.17) - 2023-11-27

### Other
- updated the following local packages: sn_networking, sn_protocol

## [0.98.16](https://github.com/maidsafe/safe_network/compare/sn_client-v0.98.15...sn_client-v0.98.16) - 2023-11-23

### Added
- *(networking)* reduce batch size to 64
- add centralised retries for all data payment kinds

### Fixed
- previous code assumptions

## [0.98.15](https://github.com/maidsafe/safe_network/compare/sn_client-v0.98.14...sn_client-v0.98.15) - 2023-11-23

### Other
- updated the following local packages: sn_networking

## [0.98.14](https://github.com/maidsafe/safe_network/compare/sn_client-v0.98.13...sn_client-v0.98.14) - 2023-11-23

### Other
- updated the following local packages: sn_transfers

## [0.98.13](https://github.com/maidsafe/safe_network/compare/sn_client-v0.98.12...sn_client-v0.98.13) - 2023-11-23

### Other
- updated the following local packages: sn_networking

## [0.98.12](https://github.com/maidsafe/safe_network/compare/sn_client-v0.98.11...sn_client-v0.98.12) - 2023-11-22

### Other
- *(release)* non gossip handler shall not throw gossip msg up

## [0.98.11](https://github.com/maidsafe/safe_network/compare/sn_client-v0.98.10...sn_client-v0.98.11) - 2023-11-22

### Added
- *(cli)* add download batch-size option

## [0.98.10](https://github.com/maidsafe/safe_network/compare/sn_client-v0.98.9...sn_client-v0.98.10) - 2023-11-21

### Added
- make joining gossip for clients and rpc nodes optional

### Other
- *(sn_networking)* enable_gossip via the builder pattern

## [0.98.9](https://github.com/maidsafe/safe_network/compare/sn_client-v0.98.8...sn_client-v0.98.9) - 2023-11-21

### Other
- updated the following local packages: sn_networking

## [0.98.8](https://github.com/maidsafe/safe_network/compare/sn_client-v0.98.7...sn_client-v0.98.8) - 2023-11-20

### Other
- increase default batch size

## [0.98.7](https://github.com/maidsafe/safe_network/compare/sn_client-v0.98.6...sn_client-v0.98.7) - 2023-11-20

### Other
- updated the following local packages: sn_networking, sn_transfers

## [0.98.6](https://github.com/maidsafe/safe_network/compare/sn_client-v0.98.5...sn_client-v0.98.6) - 2023-11-20

### Other
- updated the following local packages: sn_networking

## [0.98.5](https://github.com/maidsafe/safe_network/compare/sn_client-v0.98.4...sn_client-v0.98.5) - 2023-11-20

### Added
- quotes

## [0.98.4](https://github.com/maidsafe/safe_network/compare/sn_client-v0.98.3...sn_client-v0.98.4) - 2023-11-17

### Fixed
- *(client)* ensure we store spends at CLOSE_GROUP nodes.

## [0.98.3](https://github.com/maidsafe/safe_network/compare/sn_client-v0.98.2...sn_client-v0.98.3) - 2023-11-16

### Other
- updated the following local packages: sn_networking

## [0.98.2](https://github.com/maidsafe/safe_network/compare/sn_client-v0.98.1...sn_client-v0.98.2) - 2023-11-16

### Added
- massive cleaning to prepare for quotes

## [0.98.1](https://github.com/maidsafe/safe_network/compare/sn_client-v0.98.0...sn_client-v0.98.1) - 2023-11-15

### Other
- updated the following local packages: sn_protocol

## [0.98.0](https://github.com/maidsafe/safe_network/compare/sn_client-v0.97.6...sn_client-v0.98.0) - 2023-11-15

### Added
- *(client)* [**breaking**] error out if we cannot connect to the network in

### Other
- *(client)* [**breaking**] remove request_response timeout argument

## [0.97.6](https://github.com/maidsafe/safe_network/compare/sn_client-v0.97.5...sn_client-v0.97.6) - 2023-11-15

### Other
- updated the following local packages: sn_protocol, sn_transfers

## [0.97.5](https://github.com/maidsafe/safe_network/compare/sn_client-v0.97.4...sn_client-v0.97.5) - 2023-11-14

### Other
- *(royalties)* verify royalties fees amounts

## [0.97.4](https://github.com/maidsafe/safe_network/compare/sn_client-v0.97.3...sn_client-v0.97.4) - 2023-11-14

### Other
- updated the following local packages: sn_networking

## [0.97.3](https://github.com/maidsafe/safe_network/compare/sn_client-v0.97.2...sn_client-v0.97.3) - 2023-11-14

### Other
- updated the following local packages: sn_networking

## [0.97.2](https://github.com/maidsafe/safe_network/compare/sn_client-v0.97.1...sn_client-v0.97.2) - 2023-11-13

### Added
- no throwing up if not a gossip listener

## [0.97.1](https://github.com/maidsafe/safe_network/compare/sn_client-v0.97.0...sn_client-v0.97.1) - 2023-11-10

### Other
- updated the following local packages: sn_transfers

## [0.97.0](https://github.com/maidsafe/safe_network/compare/sn_client-v0.96.6...sn_client-v0.97.0) - 2023-11-10

### Added
- verify chunks with Quorum::N(2)
- *(client)* only pay one node

### Fixed
- *(client)* register validations checks for more than one node
- *(client)* set Quorum::One for registers
- *(test)* use client API to listen for gossipsub msgs when checking transfer notifs

### Other
- *(transfers)* more logs around payments...
- *(churn)* small delay before validating chunks in data_with_churn
- *(client)* register get quorum->one
- *(tests)* make gossipsub verification more strict wrt number of msgs received

## [0.96.6](https://github.com/maidsafe/safe_network/compare/sn_client-v0.96.5...sn_client-v0.96.6) - 2023-11-09

### Other
- updated the following local packages: sn_transfers

## [0.96.5](https://github.com/maidsafe/safe_network/compare/sn_client-v0.96.4...sn_client-v0.96.5) - 2023-11-09

### Other
- updated the following local packages: sn_networking

## [0.96.4](https://github.com/maidsafe/safe_network/compare/sn_client-v0.96.3...sn_client-v0.96.4) - 2023-11-09

### Other
- updated the following local packages: sn_networking

## [0.96.3](https://github.com/maidsafe/safe_network/compare/sn_client-v0.96.2...sn_client-v0.96.3) - 2023-11-08

### Other
- updated the following local packages: sn_networking

## [0.96.2](https://github.com/maidsafe/safe_network/compare/sn_client-v0.96.1...sn_client-v0.96.2) - 2023-11-08

### Added
- *(node)* set custom msg id in order to deduplicate transfer notifs

## [0.96.1](https://github.com/maidsafe/safe_network/compare/sn_client-v0.96.0...sn_client-v0.96.1) - 2023-11-07

### Other
- Derive Clone on ClientRegister

## [0.96.0](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.27...sn_client-v0.96.0) - 2023-11-07

### Fixed
- *(client)* [**breaking**] make `Files::chunk_file` into an associated function

## [0.95.27](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.26...sn_client-v0.95.27) - 2023-11-07

### Other
- updated the following local packages: sn_protocol

## [0.95.26](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.25...sn_client-v0.95.26) - 2023-11-06

### Added
- *(node)* log marker to track the number of peers in the routing table

## [0.95.25](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.24...sn_client-v0.95.25) - 2023-11-06

### Other
- updated the following local packages: sn_protocol

## [0.95.24](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.23...sn_client-v0.95.24) - 2023-11-06

### Other
- updated the following local packages: sn_protocol

## [0.95.23](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.22...sn_client-v0.95.23) - 2023-11-06

### Added
- *(deps)* upgrade libp2p to 0.53

## [0.95.22](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.21...sn_client-v0.95.22) - 2023-11-03

### Other
- updated the following local packages: sn_networking

## [0.95.21](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.20...sn_client-v0.95.21) - 2023-11-02

### Other
- updated the following local packages: sn_networking

## [0.95.20](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.19...sn_client-v0.95.20) - 2023-11-02

### Added
- keep transfers in mem instead of heavy cashnotes

## [0.95.19](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.18...sn_client-v0.95.19) - 2023-11-01

### Other
- updated the following local packages: sn_networking, sn_protocol

## [0.95.18](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.17...sn_client-v0.95.18) - 2023-11-01

### Other
- log detailed intermediate errors

## [0.95.17](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.16...sn_client-v0.95.17) - 2023-11-01

### Other
- updated the following local packages: sn_networking

## [0.95.16](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.15...sn_client-v0.95.16) - 2023-11-01

### Other
- updated the following local packages: sn_transfers

## [0.95.15](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.14...sn_client-v0.95.15) - 2023-10-31

### Other
- updated the following local packages: sn_networking

## [0.95.14](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.13...sn_client-v0.95.14) - 2023-10-30

### Other
- *(networking)* de/serialise directly to Bytes

## [0.95.13](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.12...sn_client-v0.95.13) - 2023-10-30

### Added
- `bincode::serialize` into `Bytes` without intermediate allocation

## [0.95.12](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.11...sn_client-v0.95.12) - 2023-10-30

### Other
- *(node)* use Bytes for Gossip related data types
- *(node)* make gossipsubpublish take Bytes

## [0.95.11](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.10...sn_client-v0.95.11) - 2023-10-27

### Added
- *(rpc-client)* be able to decrpyt received Transfers by providing a secret key

## [0.95.10](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.9...sn_client-v0.95.10) - 2023-10-27

### Other
- updated the following local packages: sn_networking

## [0.95.9](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.8...sn_client-v0.95.9) - 2023-10-26

### Fixed
- client carry out merge when verify register storage

## [0.95.8](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.7...sn_client-v0.95.8) - 2023-10-26

### Fixed
- add libp2p identity with rand dep for tests

## [0.95.7](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.6...sn_client-v0.95.7) - 2023-10-26

### Other
- updated the following local packages: sn_networking, sn_registers, sn_transfers

## [0.95.6](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.5...sn_client-v0.95.6) - 2023-10-26

### Other
- updated the following local packages: sn_networking, sn_protocol

## [0.95.5](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.4...sn_client-v0.95.5) - 2023-10-25

### Added
- *(cli)* chunk files in parallel

## [0.95.4](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.3...sn_client-v0.95.4) - 2023-10-24

### Fixed
- *(tests)* nodes rewards tests to account for repayments amounts

## [0.95.3](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.2...sn_client-v0.95.3) - 2023-10-24

### Other
- *(api)* wallet APIs to account for network royalties fees when returning total cost paid for storage

## [0.95.2](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.1...sn_client-v0.95.2) - 2023-10-24

### Other
- updated the following local packages: sn_networking, sn_transfers

## [0.95.1](https://github.com/maidsafe/safe_network/compare/sn_client-v0.95.0...sn_client-v0.95.1) - 2023-10-24

### Added
- *(client)* do not retry verification GETs

### Other
- log and debug SwarmCmd

## [0.95.0](https://github.com/maidsafe/safe_network/compare/sn_client-v0.94.8...sn_client-v0.95.0) - 2023-10-24

### Added
- *(protocol)* [**breaking**] implement `PrettyPrintRecordKey` as a `Cow` type

## [0.94.8](https://github.com/maidsafe/safe_network/compare/sn_client-v0.94.7...sn_client-v0.94.8) - 2023-10-23

### Other
- updated the following local packages: sn_networking

## [0.94.7](https://github.com/maidsafe/safe_network/compare/sn_client-v0.94.6...sn_client-v0.94.7) - 2023-10-23

### Other
- more custom debug and debug skips

## [0.94.6](https://github.com/maidsafe/safe_network/compare/sn_client-v0.94.5...sn_client-v0.94.6) - 2023-10-22

### Other
- updated the following local packages: sn_networking, sn_protocol

## [0.94.5](https://github.com/maidsafe/safe_network/compare/sn_client-v0.94.4...sn_client-v0.94.5) - 2023-10-21

### Other
- updated the following local packages: sn_networking

## [0.94.4](https://github.com/maidsafe/safe_network/compare/sn_client-v0.94.3...sn_client-v0.94.4) - 2023-10-20

### Other
- updated the following local packages: sn_networking, sn_protocol

## [0.94.3](https://github.com/maidsafe/safe_network/compare/sn_client-v0.94.2...sn_client-v0.94.3) - 2023-10-20

### Added
- *(client)* stop futher bootstrapping if the client has K_VALUE peers

## [0.94.2](https://github.com/maidsafe/safe_network/compare/sn_client-v0.94.1...sn_client-v0.94.2) - 2023-10-19

### Fixed
- *(network)* emit NetworkEvent when we publish a gossipsub msg

## [0.94.1](https://github.com/maidsafe/safe_network/compare/sn_client-v0.94.0...sn_client-v0.94.1) - 2023-10-18

### Other
- updated the following local packages: sn_networking

## [0.94.0](https://github.com/maidsafe/safe_network/compare/sn_client-v0.93.18...sn_client-v0.94.0) - 2023-10-18

### Added
- *(client)* verify register sync, and repay if not stored on all nodes
- *(client)* verify register uploads and retry and repay if failed

### Other
- Revert "feat: keep transfers in mem instead of mem and i/o heavy cashnotes"
- *(client)* always validate storage payments
- repay for data in node rewards tests
- *(client)* remove price tolerance at the client

## [0.93.18](https://github.com/maidsafe/safe_network/compare/sn_client-v0.93.17...sn_client-v0.93.18) - 2023-10-18

### Added
- keep transfers in mem instead of mem and i/o heavy cashnotes

## [0.93.17](https://github.com/maidsafe/safe_network/compare/sn_client-v0.93.16...sn_client-v0.93.17) - 2023-10-17

### Fixed
- *(transfers)* dont overwrite existing payment transactions when we top up

### Other
- adding comments and cleanup around quorum / payment fixes

## [0.93.16](https://github.com/maidsafe/safe_network/compare/sn_client-v0.93.15...sn_client-v0.93.16) - 2023-10-16

### Fixed
- return correct error type
- consider record split an error, handle it for regs

## [0.93.15](https://github.com/maidsafe/safe_network/compare/sn_client-v0.93.14...sn_client-v0.93.15) - 2023-10-16

### Other
- updated the following local packages: sn_networking

## [0.93.14](https://github.com/maidsafe/safe_network/compare/sn_client-v0.93.13...sn_client-v0.93.14) - 2023-10-13

### Other
- updated the following local packages: sn_networking, sn_protocol

## [0.93.13](https://github.com/maidsafe/safe_network/compare/sn_client-v0.93.12...sn_client-v0.93.13) - 2023-10-13

### Fixed
- batch download process

## [0.93.12](https://github.com/maidsafe/safe_network/compare/sn_client-v0.93.11...sn_client-v0.93.12) - 2023-10-12

### Other
- updated the following local packages: sn_networking

## [0.93.11](https://github.com/maidsafe/safe_network/compare/sn_client-v0.93.10...sn_client-v0.93.11) - 2023-10-12

### Other
- updated the following local packages: sn_networking

## [0.93.10](https://github.com/maidsafe/safe_network/compare/sn_client-v0.93.9...sn_client-v0.93.10) - 2023-10-12

### Other
- more detailed logging when client creating store cash_note

## [0.93.9](https://github.com/maidsafe/safe_network/compare/sn_client-v0.93.8...sn_client-v0.93.9) - 2023-10-11

### Fixed
- expose RecordMismatch errors and cleanup wallet if we hit that

### Other
- *(transfers)* add somre more clarity around DoubleSpendAttemptedForCashNotes
- *(transfers)* remove pointless api

## [0.93.8](https://github.com/maidsafe/safe_network/compare/sn_client-v0.93.7...sn_client-v0.93.8) - 2023-10-11

### Other
- updated the following local packages: sn_networking

## [0.93.7](https://github.com/maidsafe/safe_network/compare/sn_client-v0.93.6...sn_client-v0.93.7) - 2023-10-11

### Added
- showing expected holders to CLI when required
- verify put_record with expected_holders

## [0.93.6](https://github.com/maidsafe/safe_network/compare/sn_client-v0.93.5...sn_client-v0.93.6) - 2023-10-10

### Added
- *(transfer)* special event for transfer notifs over gossipsub

## [0.93.5](https://github.com/maidsafe/safe_network/compare/sn_client-v0.93.4...sn_client-v0.93.5) - 2023-10-10

### Other
- compare files after download twice

## [0.93.4](https://github.com/maidsafe/safe_network/compare/sn_client-v0.93.3...sn_client-v0.93.4) - 2023-10-10

### Other
- updated the following local packages: sn_transfers

## [0.93.3](https://github.com/maidsafe/safe_network/compare/sn_client-v0.93.2...sn_client-v0.93.3) - 2023-10-09

### Other
- updated the following local packages: sn_networking

## [0.93.2](https://github.com/maidsafe/safe_network/compare/sn_client-v0.93.1...sn_client-v0.93.2) - 2023-10-08

### Other
- updated the following local packages: sn_networking

## [0.93.1](https://github.com/maidsafe/safe_network/compare/sn_client-v0.93.0...sn_client-v0.93.1) - 2023-10-06

### Added
- feat!(sn_transfers): unify store api for wallet

### Other
- *(client)* dont println for wallet errors

## [0.93.0](https://github.com/maidsafe/safe_network/compare/sn_client-v0.92.9...sn_client-v0.93.0) - 2023-10-06

### Fixed
- *(client)* [**breaking**] unify send_without_verify and send functions

### Other
- *(cli)* reuse the client::send function to send amount from wallet

## [0.92.9](https://github.com/maidsafe/safe_network/compare/sn_client-v0.92.8...sn_client-v0.92.9) - 2023-10-06

### Other
- fix new clippy errors

## [0.92.8](https://github.com/maidsafe/safe_network/compare/sn_client-v0.92.7...sn_client-v0.92.8) - 2023-10-05

### Other
- updated the following local packages: sn_networking, sn_transfers

## [0.92.7](https://github.com/maidsafe/safe_network/compare/sn_client-v0.92.6...sn_client-v0.92.7) - 2023-10-05

### Added
- feat!(cli): remove concurrency argument

## [0.92.6](https://github.com/maidsafe/safe_network/compare/sn_client-v0.92.5...sn_client-v0.92.6) - 2023-10-05

### Fixed
- *(sn_transfers)* be sure we store CashNotes before writing the wallet file

## [0.92.5](https://github.com/maidsafe/safe_network/compare/sn_client-v0.92.4...sn_client-v0.92.5) - 2023-10-05

### Added
- quorum for records get

### Fixed
- use specific verify func for chunk stored verification

## [0.92.4](https://github.com/maidsafe/safe_network/compare/sn_client-v0.92.3...sn_client-v0.92.4) - 2023-10-05

### Added
- use progress bars on `files upload`

### Other
- pay_for_chunks returns cost and new balance

## [0.92.3](https://github.com/maidsafe/safe_network/compare/sn_client-v0.92.2...sn_client-v0.92.3) - 2023-10-04

### Fixed
- *(wallet)* remove expect statments

## [0.92.2](https://github.com/maidsafe/safe_network/compare/sn_client-v0.92.1...sn_client-v0.92.2) - 2023-10-04

### Fixed
- record_to_verify for store_chunk shall be a Chunk

## [0.92.1](https://github.com/maidsafe/safe_network/compare/sn_client-v0.92.0...sn_client-v0.92.1) - 2023-10-04

### Other
- updated the following local packages: sn_networking

## [0.92.0](https://github.com/maidsafe/safe_network/compare/sn_client-v0.91.11...sn_client-v0.92.0) - 2023-10-04

### Added
- improve register API

## [0.91.11](https://github.com/maidsafe/safe_network/compare/sn_client-v0.91.10...sn_client-v0.91.11) - 2023-10-04

### Added
- *(client)* reuse cashnotes for address payments

### Other
- separate method and write test

## [0.91.10](https://github.com/maidsafe/safe_network/compare/sn_client-v0.91.9...sn_client-v0.91.10) - 2023-10-03

### Other
- updated the following local packages: sn_networking

## [0.91.9](https://github.com/maidsafe/safe_network/compare/sn_client-v0.91.8...sn_client-v0.91.9) - 2023-10-03

### Added
- re-attempt when get chunk from network

## [0.91.8](https://github.com/maidsafe/safe_network/compare/sn_client-v0.91.7...sn_client-v0.91.8) - 2023-10-03

### Other
- updated the following local packages: sn_networking

## [0.91.7](https://github.com/maidsafe/safe_network/compare/sn_client-v0.91.6...sn_client-v0.91.7) - 2023-10-02

### Other
- remove all spans.

## [0.91.6](https://github.com/maidsafe/safe_network/compare/sn_client-v0.91.5...sn_client-v0.91.6) - 2023-10-02

### Other
- updated the following local packages: sn_transfers

## [0.91.5](https://github.com/maidsafe/safe_network/compare/sn_client-v0.91.4...sn_client-v0.91.5) - 2023-10-02

### Other
- *(client)* more logs around StoreCost retrieveal

## [0.91.4](https://github.com/maidsafe/safe_network/compare/sn_client-v0.91.3...sn_client-v0.91.4) - 2023-09-29

### Other
- updated the following local packages: sn_networking, sn_protocol

## [0.91.3](https://github.com/maidsafe/safe_network/compare/sn_client-v0.91.2...sn_client-v0.91.3) - 2023-09-28

### Added
- client to client transfers

## [0.91.2](https://github.com/maidsafe/safe_network/compare/sn_client-v0.91.1...sn_client-v0.91.2) - 2023-09-27

### Added
- *(networking)* remove optional_semaphore being passed down from apps
- all records are Quorum::All once more

## [0.91.1](https://github.com/maidsafe/safe_network/compare/sn_client-v0.91.0...sn_client-v0.91.1) - 2023-09-27

### Added
- *(client)* fail fast when a chunk is missing

## [0.91.0](https://github.com/maidsafe/safe_network/compare/sn_client-v0.90.6...sn_client-v0.91.0) - 2023-09-27

### Added
- deep clean sn_transfers, reduce exposition, remove dead code

## [0.90.6](https://github.com/maidsafe/safe_network/compare/sn_client-v0.90.5...sn_client-v0.90.6) - 2023-09-26

### Other
- updated the following local packages: sn_networking

## [0.90.5](https://github.com/maidsafe/safe_network/compare/sn_client-v0.90.4...sn_client-v0.90.5) - 2023-09-26

### Added
- *(apis)* adding client and node APIs, as well as safenode RPC service to unsubscribe from gossipsub topics

## [0.90.4](https://github.com/maidsafe/safe_network/compare/sn_client-v0.90.3...sn_client-v0.90.4) - 2023-09-25

### Other
- updated the following local packages: sn_transfers

## [0.90.3](https://github.com/maidsafe/safe_network/compare/sn_client-v0.90.2...sn_client-v0.90.3) - 2023-09-25

### Other
- cleanup renamings in sn_transfers

## [0.90.2](https://github.com/maidsafe/safe_network/compare/sn_client-v0.90.1...sn_client-v0.90.2) - 2023-09-25

### Other
- *(client)* serialize ClientEvent

## [0.90.1](https://github.com/maidsafe/safe_network/compare/sn_client-v0.90.0...sn_client-v0.90.1) - 2023-09-22

### Added
- *(apis)* adding client and node APIs, as well as safenode RPC services to pub/sub to gossipsub topics

## [0.90.0](https://github.com/maidsafe/safe_network/compare/sn_client-v0.89.23...sn_client-v0.90.0) - 2023-09-21

### Added
- dusking DBCs

### Other
- rename Nano NanoTokens
- improve naming

## [0.89.23](https://github.com/maidsafe/safe_network/compare/sn_client-v0.89.22...sn_client-v0.89.23) - 2023-09-21

### Other
- updated the following local packages: sn_networking

## [0.89.22](https://github.com/maidsafe/safe_network/compare/sn_client-v0.89.21...sn_client-v0.89.22) - 2023-09-21

### Other
- clarify `files download` usage
- output address of uploaded file

## [0.89.21](https://github.com/maidsafe/safe_network/compare/sn_client-v0.89.20...sn_client-v0.89.21) - 2023-09-20

### Other
- updated the following local packages: sn_networking

## [0.89.20](https://github.com/maidsafe/safe_network/compare/sn_client-v0.89.19...sn_client-v0.89.20) - 2023-09-20

### Other
- major dep updates

## [0.89.19](https://github.com/maidsafe/safe_network/compare/sn_client-v0.89.18...sn_client-v0.89.19) - 2023-09-20

### Other
- allow chunks to be Quorum::One

## [0.89.18](https://github.com/maidsafe/safe_network/compare/sn_client-v0.89.17...sn_client-v0.89.18) - 2023-09-19

### Other
- updated the following local packages: sn_networking

## [0.89.17](https://github.com/maidsafe/safe_network/compare/sn_client-v0.89.16...sn_client-v0.89.17) - 2023-09-19

### Other
- error handling when failed fetch store cost

## [0.89.16](https://github.com/maidsafe/safe_network/compare/sn_client-v0.89.15...sn_client-v0.89.16) - 2023-09-19

### Other
- updated the following local packages: sn_networking

## [0.89.15](https://github.com/maidsafe/safe_network/compare/sn_client-v0.89.14...sn_client-v0.89.15) - 2023-09-19

### Other
- updated the following local packages: sn_networking

## [0.89.14](https://github.com/maidsafe/safe_network/compare/sn_client-v0.89.13...sn_client-v0.89.14) - 2023-09-18

### Other
- updated the following local packages: sn_networking

## [0.89.13](https://github.com/maidsafe/safe_network/compare/sn_client-v0.89.12...sn_client-v0.89.13) - 2023-09-18

### Added
- *(client)* download file concurrently

## [0.89.12](https://github.com/maidsafe/safe_network/compare/sn_client-v0.89.11...sn_client-v0.89.12) - 2023-09-18

### Added
- serialisation for transfers for out of band sending

### Other
- *(client)* simplify API
- *(cli)* use iter::chunks() API to batch and pay for our chunks

## [0.89.11](https://github.com/maidsafe/safe_network/compare/sn_client-v0.89.10...sn_client-v0.89.11) - 2023-09-15

### Added
- *(client)* pay for chunks in batches

### Other
- *(client)* refactor chunk upload code to allow greater concurrency

## [0.89.10](https://github.com/maidsafe/safe_network/compare/sn_client-v0.89.9...sn_client-v0.89.10) - 2023-09-15

### Other
- updated the following local packages: sn_networking, sn_transfers

## [0.89.9](https://github.com/maidsafe/safe_network/compare/sn_client-v0.89.8...sn_client-v0.89.9) - 2023-09-15

### Other
- *(client)* remove unused wallet_client

## [0.89.8](https://github.com/maidsafe/safe_network/compare/sn_client-v0.89.7...sn_client-v0.89.8) - 2023-09-14

### Added
- *(register)* client to pay for Register only if local wallet has not paymnt for it yet

## [0.89.7](https://github.com/maidsafe/safe_network/compare/sn_client-v0.89.6...sn_client-v0.89.7) - 2023-09-14

### Added
- split upload procedure into batches

## [0.89.6](https://github.com/maidsafe/safe_network/compare/sn_client-v0.89.5...sn_client-v0.89.6) - 2023-09-14

### Added
- *(network)* enable custom node metrics
- *(network)* use NetworkConfig for network construction

### Other
- remove unused error variants
- *(network)* use builder pattern to construct the Network
- *(metrics)* rename feature flag and small fixes

## [0.89.5](https://github.com/maidsafe/safe_network/compare/sn_client-v0.89.4...sn_client-v0.89.5) - 2023-09-13

### Added
- *(register)* paying nodes for Register storage

### Other
- *(register)* adding Register payment storage tests to run in CI
- *(payments)* adaptig code to recent changes in Transfers

## [0.89.4](https://github.com/maidsafe/safe_network/compare/sn_client-v0.89.3...sn_client-v0.89.4) - 2023-09-12

### Added
- utilize stream decryptor

## [0.89.3](https://github.com/maidsafe/safe_network/compare/sn_client-v0.89.2...sn_client-v0.89.3) - 2023-09-12

### Other
- updated the following local packages: sn_networking

## [0.89.2](https://github.com/maidsafe/safe_network/compare/sn_client-v0.89.1...sn_client-v0.89.2) - 2023-09-12

### Other
- *(metrics)* rename network metrics and remove from default features list

## [0.89.1](https://github.com/maidsafe/safe_network/compare/sn_client-v0.89.0...sn_client-v0.89.1) - 2023-09-12

### Added
- add tx and parent spends verification
- chunk payments using UTXOs instead of DBCs

### Other
- use updated sn_dbc

## [0.89.0](https://github.com/maidsafe/safe_network/compare/sn_client-v0.88.16...sn_client-v0.89.0) - 2023-09-11

### Added
- [**breaking**] Clients add a tolerance to store cost

## [0.88.16](https://github.com/maidsafe/safe_network/compare/sn_client-v0.88.15...sn_client-v0.88.16) - 2023-09-11

### Other
- utilize stream encryptor

## [0.88.15](https://github.com/maidsafe/safe_network/compare/sn_client-v0.88.14...sn_client-v0.88.15) - 2023-09-08

### Added
- *(client)* repay for chunks if they cannot be validated

### Other
- *(client)* refactor to have permits at network layer
- *(refactor)* remove wallet_client args from upload flow
- *(refactor)* remove upload_chunks semaphore arg

## [0.88.14](https://github.com/maidsafe/safe_network/compare/sn_client-v0.88.13...sn_client-v0.88.14) - 2023-09-07

### Other
- updated the following local packages: sn_networking

## [0.88.13](https://github.com/maidsafe/safe_network/compare/sn_client-v0.88.12...sn_client-v0.88.13) - 2023-09-07

### Other
- updated the following local packages: sn_networking

## [0.88.12](https://github.com/maidsafe/safe_network/compare/sn_client-v0.88.11...sn_client-v0.88.12) - 2023-09-05

### Other
- updated the following local packages: sn_networking, sn_transfers

## [0.88.11](https://github.com/maidsafe/safe_network/compare/sn_client-v0.88.10...sn_client-v0.88.11) - 2023-09-05

### Added
- encryptioni output to disk

## [0.88.10](https://github.com/maidsafe/safe_network/compare/sn_client-v0.88.9...sn_client-v0.88.10) - 2023-09-05

### Other
- updated the following local packages: sn_networking

## [0.88.9](https://github.com/maidsafe/safe_network/compare/sn_client-v0.88.8...sn_client-v0.88.9) - 2023-09-04

### Added
- feat!(protocol): make payments for all record types

### Fixed
- fix permissions for public register creation

### Other
- *(release)* sn_registers-v0.2.4
- utilize encrypt_from_file

## [0.88.8](https://github.com/maidsafe/safe_network/compare/sn_client-v0.88.7...sn_client-v0.88.8) - 2023-09-04

### Other
- Add client and protocol detail

## [0.88.7](https://github.com/maidsafe/safe_network/compare/sn_client-v0.88.6...sn_client-v0.88.7) - 2023-09-01

### Other
- *(transfers)* store dbcs by ref to avoid more clones
- *(client)* make unconfonfirmed txs btreeset, remove unnecessary cloning
- *(client)* remove one signed_spend clone

## [0.88.6](https://github.com/maidsafe/safe_network/compare/sn_client-v0.88.5...sn_client-v0.88.6) - 2023-09-01

### Other
- updated the following local packages: sn_networking

## [0.88.5](https://github.com/maidsafe/safe_network/compare/sn_client-v0.88.4...sn_client-v0.88.5) - 2023-08-31

### Other
- remove unused async

## [0.88.4](https://github.com/maidsafe/safe_network/compare/sn_client-v0.88.3...sn_client-v0.88.4) - 2023-08-31

### Other
- updated the following local packages: sn_protocol, sn_transfers

## [0.88.3](https://github.com/maidsafe/safe_network/compare/sn_client-v0.88.2...sn_client-v0.88.3) - 2023-08-31

### Other
- some logging updates

## [0.88.2](https://github.com/maidsafe/safe_network/compare/sn_client-v0.88.1...sn_client-v0.88.2) - 2023-08-31

### Other
- updated the following local packages: sn_networking, sn_protocol

## [0.88.1](https://github.com/maidsafe/safe_network/compare/sn_client-v0.88.0...sn_client-v0.88.1) - 2023-08-31

### Added
- *(cli)* expose 'concurrency' flag
- *(cli)* increase put parallelisation

### Other
- *(client)* reduce default concurrency
- *(client)* improve download concurrency.

## [0.88.0](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.29...sn_client-v0.88.0) - 2023-08-30

### Added
- refactor to allow greater upload parallelisation
- one transfer per data set, mapped dbcs to content addrs
- [**breaking**] pay each chunk holder direct
- feat!(protocol): gets keys with GetStoreCost
- feat!(protocol): get price and pay for each chunk individually
- feat!(protocol): remove chunk merkletree to simplify payment

### Fixed
- *(tokio)* remove tokio fs

### Other
- *(node)* refactor churn test order
- *(deps)* bump tokio to 1.32.0
- *(client)* refactor client wallet to reduce dbc clones
- *(client)* pass around content payments map mut ref
- *(client)* reduce transferoutputs cloning
- *(client)* error out early for invalid transfers
- *(node)* reenable payment fail check

## [0.87.29](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.28...sn_client-v0.87.29) - 2023-08-30

### Other
- updated the following local packages: sn_networking

## [0.87.28](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.27...sn_client-v0.87.28) - 2023-08-29

### Other
- updated the following local packages: sn_networking

## [0.87.27](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.26...sn_client-v0.87.27) - 2023-08-24

### Other
- updated the following local packages: sn_registers, sn_transfers

## [0.87.26](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.25...sn_client-v0.87.26) - 2023-08-22

### Other
- updated the following local packages: sn_networking

## [0.87.25](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.24...sn_client-v0.87.25) - 2023-08-22

### Fixed
- fixes to allow upload file works properly

## [0.87.24](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.23...sn_client-v0.87.24) - 2023-08-21

### Other
- updated the following local packages: sn_networking

## [0.87.23](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.22...sn_client-v0.87.23) - 2023-08-21

### Other
- updated the following local packages: sn_networking

## [0.87.22](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.21...sn_client-v0.87.22) - 2023-08-18

### Added
- remove client and node initial join flow

## [0.87.21](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.20...sn_client-v0.87.21) - 2023-08-18

### Other
- updated the following local packages: sn_protocol

## [0.87.20](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.19...sn_client-v0.87.20) - 2023-08-17

### Fixed
- *(client)* start bootstrap when we are connected to one peer

## [0.87.19](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.18...sn_client-v0.87.19) - 2023-08-17

### Other
- updated the following local packages: sn_networking

## [0.87.18](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.17...sn_client-v0.87.18) - 2023-08-17

### Fixed
- *(client)* use boostrap and fire Connecting event

## [0.87.17](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.16...sn_client-v0.87.17) - 2023-08-17

### Other
- updated the following local packages: sn_networking

## [0.87.16](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.15...sn_client-v0.87.16) - 2023-08-16

### Added
- *(client)* do not use cached proofs

## [0.87.15](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.14...sn_client-v0.87.15) - 2023-08-16

### Added
- overpay by default to allow margin

## [0.87.14](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.13...sn_client-v0.87.14) - 2023-08-15

### Other
- updated the following local packages: sn_networking

## [0.87.13](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.12...sn_client-v0.87.13) - 2023-08-11

### Added
- *(transfers)* add resend loop for unconfirmed txs
- *(networking)* ensure we always use the highest price we find
- *(networking)* enable returning less than majority for store_cost
- *(client)* use store cost queries to pre populate cost and RT

### Fixed
- *(client)* only_store_cost_if_higher missing else added

### Other
- remove client inactivity random storage query
- *(node)* resend unconfirmed txs before asserting
- *(cli)* print cost info
- *(networking)* remove logs, fix typos and clippy issues
- overpay in advance to avoid storage cost calculation inconsistent

## [0.87.12](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.11...sn_client-v0.87.12) - 2023-08-10

### Other
- updated the following local packages: sn_networking, sn_protocol

## [0.87.11](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.10...sn_client-v0.87.11) - 2023-08-10

### Other
- updated the following local packages: sn_networking

## [0.87.10](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.9...sn_client-v0.87.10) - 2023-08-08

### Added
- *(transfers)* add get largest dbc for spending

### Fixed
- *(node)* prevent panic in storage calcs

### Other
- tidy store cost code

## [0.87.9](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.8...sn_client-v0.87.9) - 2023-08-07

### Other
- updated the following local packages: sn_networking

## [0.87.8](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.7...sn_client-v0.87.8) - 2023-08-07

### Added
- rework register addresses to include pk

### Other
- rename network addresses confusing name method to xorname

## [0.87.7](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.6...sn_client-v0.87.7) - 2023-08-04

### Other
- updated the following local packages: sn_networking

## [0.87.6](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.5...sn_client-v0.87.6) - 2023-08-03

### Other
- updated the following local packages: sn_networking

## [0.87.5](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.4...sn_client-v0.87.5) - 2023-08-03

### Other
- updated the following local packages: sn_networking

## [0.87.4](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.3...sn_client-v0.87.4) - 2023-08-02

### Fixed
- do not create genesis when facuet already funded

## [0.87.3](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.2...sn_client-v0.87.3) - 2023-08-01

### Other
- *(client)* reattempt to get_spend_from_network
- add more verificaiton for payments

## [0.87.2](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.1...sn_client-v0.87.2) - 2023-08-01

### Other
- updated the following local packages: sn_protocol

## [0.87.1](https://github.com/maidsafe/safe_network/compare/sn_client-v0.87.0...sn_client-v0.87.1) - 2023-08-01

### Added
- *(cli)* add no-verify flag to cli

### Other
- fix double spend and remove arbitrary wait
- *(node)* verify faucet transactions before continuing
- *(netowrking)* change default re-attempt behaviour

## [0.87.0](https://github.com/maidsafe/safe_network/compare/sn_client-v0.86.11...sn_client-v0.87.0) - 2023-08-01

### Other
- *(register)* [**breaking**] hashing the node of a Register to sign it instead of bincode-serialising it

## [0.86.11](https://github.com/maidsafe/safe_network/compare/sn_client-v0.86.10...sn_client-v0.86.11) - 2023-07-31

### Other
- updated the following local packages: sn_networking

## [0.86.10](https://github.com/maidsafe/safe_network/compare/sn_client-v0.86.9...sn_client-v0.86.10) - 2023-07-31

### Added
- carry out get_record re-attempts for critical record
- for put_record verification, NotEnoughCopies is acceptable

### Fixed
- *(test)* using proper wallets during data_with_churn test

### Other
- move PrettyPrintRecordKey to sn_protocol
- small refactors for failing CI
- more tracable logs regarding chunk payment prove

## [0.86.9](https://github.com/maidsafe/safe_network/compare/sn_client-v0.86.8...sn_client-v0.86.9) - 2023-07-31

### Other
- updated the following local packages: sn_networking

## [0.86.8](https://github.com/maidsafe/safe_network/compare/sn_client-v0.86.7...sn_client-v0.86.8) - 2023-07-28

### Other
- updated the following local packages: sn_networking

## [0.86.7](https://github.com/maidsafe/safe_network/compare/sn_client-v0.86.6...sn_client-v0.86.7) - 2023-07-28

### Other
- updated the following local packages: sn_networking, sn_protocol

## [0.86.6](https://github.com/maidsafe/safe_network/compare/sn_client-v0.86.5...sn_client-v0.86.6) - 2023-07-28

### Other
- adapt all logging to use pretty record key

## [0.86.5](https://github.com/maidsafe/safe_network/compare/sn_client-v0.86.4...sn_client-v0.86.5) - 2023-07-27

### Other
- updated the following local packages: sn_networking

## [0.86.4](https://github.com/maidsafe/safe_network/compare/sn_client-v0.86.3...sn_client-v0.86.4) - 2023-07-26

### Fixed
- *(register)* Registers with same name but different tags were not being stored by the network

### Other
- centralising RecordKey creation logic to make sure we always use the same for all content type

## [0.86.3](https://github.com/maidsafe/safe_network/compare/sn_client-v0.86.2...sn_client-v0.86.3) - 2023-07-26

### Other
- updated the following local packages: sn_networking

## [0.86.2](https://github.com/maidsafe/safe_network/compare/sn_client-v0.86.1...sn_client-v0.86.2) - 2023-07-26

### Other
- updated the following local packages: sn_networking

## [0.86.1](https://github.com/maidsafe/safe_network/compare/sn_client-v0.86.0...sn_client-v0.86.1) - 2023-07-25

### Added
- *(replication)* replicate when our close group changes

### Fixed
- *(client)* keep an active `ClientEvent` receiver

### Other
- *(client)* get k_value from const fn

## [0.86.0](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.55...sn_client-v0.86.0) - 2023-07-21

### Added
- *(protocol)* [**breaking**] make Chunks storage payment required

### Other
- tokens transfers task in data_with_churn tests to use client apis instead of faucet helpers

## [0.85.55](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.54...sn_client-v0.85.55) - 2023-07-20

### Other
- cleanup error types

## [0.85.54](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.53...sn_client-v0.85.54) - 2023-07-19

### Added
- using kad::record for dbc spend ops
- *(CI)* dbc verfication during network churning test

## [0.85.53](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.52...sn_client-v0.85.53) - 2023-07-19

### Other
- updated the following local packages: sn_protocol

## [0.85.52](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.51...sn_client-v0.85.52) - 2023-07-18

### Other
- updated the following local packages: sn_networking

## [0.85.51](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.50...sn_client-v0.85.51) - 2023-07-18

### Added
- safer registers requiring signatures
- *(networking)* remove LostRecordEvent

### Fixed
- address PR comments
- client

## [0.85.50](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.49...sn_client-v0.85.50) - 2023-07-18

### Other
- updated the following local packages: sn_networking

## [0.85.49](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.48...sn_client-v0.85.49) - 2023-07-17

### Other
- updated the following local packages: sn_networking

## [0.85.48](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.47...sn_client-v0.85.48) - 2023-07-17

### Added
- *(networking)* upgrade to libp2p 0.52.0

### Other
- *(networking)* log all connected peer count

## [0.85.47](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.46...sn_client-v0.85.47) - 2023-07-17

### Added
- *(client)* keep storage payment proofs in local wallet

## [0.85.46](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.45...sn_client-v0.85.46) - 2023-07-12

### Other
- client to upload paid chunks in batches

## [0.85.45](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.44...sn_client-v0.85.45) - 2023-07-11

### Other
- updated the following local packages: sn_networking

## [0.85.44](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.43...sn_client-v0.85.44) - 2023-07-11

### Fixed
- *(client)* publish register on creation

## [0.85.43](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.42...sn_client-v0.85.43) - 2023-07-11

### Other
- updated the following local packages: sn_networking

## [0.85.42](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.41...sn_client-v0.85.42) - 2023-07-10

### Other
- updated the following local packages: sn_networking

## [0.85.41](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.40...sn_client-v0.85.41) - 2023-07-10

### Added
- client query register via get_record
- client upload Register via put_record

## [0.85.40](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.39...sn_client-v0.85.40) - 2023-07-06

### Other
- updated the following local packages: sn_networking

## [0.85.39](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.38...sn_client-v0.85.39) - 2023-07-06

### Added
- PutRecord response during client upload
- client upload chunk using kad::put_record

### Other
- *(release)* sn_cli-v0.79.0/sn_logging-v0.2.0/sn_node-v0.86.0/sn_testnet-v0.1.76/sn_networking-v0.3.11

## [0.85.38](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.37...sn_client-v0.85.38) - 2023-07-05

### Added
- carry out validation for record_store::put

## [0.85.37](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.36...sn_client-v0.85.37) - 2023-07-04

### Other
- demystify permissions

## [0.85.36](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.35...sn_client-v0.85.36) - 2023-07-03

### Added
- append SAFE_PEERS to initial_peers after restart

### Fixed
- *(text)* data_churn_test creates clients parsing SAFE_PEERS env

### Other
- reduce SAMPLE_SIZE for the data_with_churn test
- some client log tidy up

## [0.85.35](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.34...sn_client-v0.85.35) - 2023-06-29

### Other
- updated the following local packages: sn_networking

## [0.85.34](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.33...sn_client-v0.85.34) - 2023-06-28

### Other
- updated the following local packages: sn_networking

## [0.85.33](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.32...sn_client-v0.85.33) - 2023-06-28

### Added
- make the example work, fix sync when reg doesnt exist
- rework permissions, implement register cmd handlers
- register refactor, kad reg without cmds

### Fixed
- rename UserRights to UserPermissions

## [0.85.32](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.31...sn_client-v0.85.32) - 2023-06-28

### Other
- updated the following local packages: sn_networking

## [0.85.31](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.30...sn_client-v0.85.31) - 2023-06-28

### Added
- *(node)* dial without PeerId

## [0.85.30](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.29...sn_client-v0.85.30) - 2023-06-27

### Other
- updated the following local packages: sn_networking

## [0.85.29](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.28...sn_client-v0.85.29) - 2023-06-27

### Other
- updated the following local packages: sn_networking

## [0.85.28](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.27...sn_client-v0.85.28) - 2023-06-26

### Other
- updated the following local packages: sn_networking

## [0.85.27](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.26...sn_client-v0.85.27) - 2023-06-26

### Other
- updated the following local packages: sn_networking

## [0.85.26](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.25...sn_client-v0.85.26) - 2023-06-26

### Other
- *(release)* sn_cli-v0.78.9/sn_logging-v0.1.4/sn_node-v0.83.55/sn_testnet-v0.1.59/sn_networking-v0.1.24

## [0.85.25](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.24...sn_client-v0.85.25) - 2023-06-26

### Other
- payment proof map to use xorname as index instead of merkletree nodes type

## [0.85.24](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.23...sn_client-v0.85.24) - 2023-06-24

### Other
- updated the following local packages: sn_networking

## [0.85.23](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.22...sn_client-v0.85.23) - 2023-06-23

### Other
- updated the following local packages: sn_networking

## [0.85.22](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.21...sn_client-v0.85.22) - 2023-06-23

### Added
- forward chunk when not being the closest
- repliate to peers lost record

### Fixed
- client upload to peers closer to chunk

## [0.85.21](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.20...sn_client-v0.85.21) - 2023-06-23

### Other
- updated the following local packages: sn_networking

## [0.85.20](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.19...sn_client-v0.85.20) - 2023-06-22

### Other
- *(client)* initial refactor around uploads

## [0.85.19](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.18...sn_client-v0.85.19) - 2023-06-22

### Fixed
- improve client upload speed

## [0.85.18](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.17...sn_client-v0.85.18) - 2023-06-21

### Other
- updated the following local packages: sn_networking, sn_protocol

## [0.85.17](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.16...sn_client-v0.85.17) - 2023-06-21

### Other
- *(network)* remove `NetworkEvent::PutRecord` dead code

## [0.85.16](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.15...sn_client-v0.85.16) - 2023-06-21

### Other
- remove unused error variants
- *(node)* obtain parent_tx from SignedSpend
- *(release)* sn_cli-v0.77.46/sn_logging-v0.1.3/sn_node-v0.83.42/sn_testnet-v0.1.46/sn_networking-v0.1.15

## [0.85.15](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.14...sn_client-v0.85.15) - 2023-06-20

### Added
- *(network)* validate `Record` on GET
- *(network)* validate and store `ReplicatedData`
- *(node)* perform proper validations on PUT
- *(network)* validate and store `Record`

### Fixed
- *(node)* store parent tx along with `SignedSpend`

### Other
- *(docs)* add more docs and comments

## [0.85.14](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.13...sn_client-v0.85.14) - 2023-06-20

### Other
- updated the following local packages: sn_networking

## [0.85.13](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.12...sn_client-v0.85.13) - 2023-06-20

### Added
- pay 1 nano per Chunk as temporary approach till net-invoices are implemented
- committing storage payment SignedSpends to the network
- nodes to verify input DBCs of Chunk payment proof were spent

### Other
- specific error types for different payment proof verification scenarios
- include the Tx instead of output DBCs as part of storage payment proofs

## [0.85.12](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.11...sn_client-v0.85.12) - 2023-06-20

### Other
- updated the following local packages: sn_networking

## [0.85.11](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.10...sn_client-v0.85.11) - 2023-06-16

### Fixed
- reduce client mem usage during uploading

## [0.85.10](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.9...sn_client-v0.85.10) - 2023-06-15

### Added
- add double spend test

### Fixed
- parent spend issue

## [0.85.9](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.8...sn_client-v0.85.9) - 2023-06-14

### Added
- include output DBC within payment proof for Chunks storage

## [0.85.8](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.7...sn_client-v0.85.8) - 2023-06-14

### Other
- updated the following local packages: sn_networking

## [0.85.7](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.6...sn_client-v0.85.7) - 2023-06-14

### Added
- *(client)* expose req/resp timeout to client cli

## [0.85.6](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.5...sn_client-v0.85.6) - 2023-06-13

### Other
- *(release)* sn_cli-v0.77.12/sn_logging-v0.1.2/sn_node-v0.83.10/sn_testnet-v0.1.14/sn_networking-v0.1.6

## [0.85.5](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.4...sn_client-v0.85.5) - 2023-06-12

### Added
- remove spendbook rw locks, improve logging

### Other
- remove uneeded printlns
- *(release)* sn_cli-v0.77.10/sn_record_store-v0.1.3/sn_node-v0.83.8/sn_testnet-v0.1.12/sn_networking-v0.1.4

## [0.85.4](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.3...sn_client-v0.85.4) - 2023-06-09

### Other
- manually change crate version

## [0.85.3](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.2...sn_client-v0.85.3) - 2023-06-09

### Other
- more replication flow statistics during mem_check test

## [0.85.2](https://github.com/maidsafe/safe_network/compare/sn_client-v0.85.1...sn_client-v0.85.2) - 2023-06-07

### Added
- bail out if empty list of addreses is provided for payment proof generation
- *(client)* add progress indicator for initial network connections
- attach payment proof when uploading Chunks
- collect payment proofs and make sure merkletree always has pow-of-2 leaves
- node side payment proof validation from a given Chunk, audit trail, and reason-hash
- use all Chunks of a file to generate payment the payment proof tree
- Chunk storage payment and building payment proofs

### Fixed
- remove progress bar after it's finished.

### Other
- Revert "chore(release): sn_cli-v0.77.1/sn_client-v0.85.2/sn_networking-v0.1.2/sn_node-v0.83.1"
- *(release)* sn_cli-v0.77.1/sn_client-v0.85.2/sn_networking-v0.1.2/sn_node-v0.83.1
- Revert "chore(release): sn_cli-v0.77.1/sn_client-v0.85.2/sn_networking-v0.1.2/sn_protocol-v0.1.2/sn_node-v0.83.1/sn_record_store-v0.1.2/sn_registers-v0.1.2"
- *(release)* sn_cli-v0.77.1/sn_client-v0.85.2/sn_networking-v0.1.2/sn_protocol-v0.1.2/sn_node-v0.83.1/sn_record_store-v0.1.2/sn_registers-v0.1.2
- small log wording updates
- exposing definition of merkletree nodes data type and additional doc in code
- making Chunk payment proof optional for now
- moving all payment proofs utilities into sn_transfers crate

## [0.85.1](https://github.com/jacderida/safe_network/compare/sn_client-v0.85.0...sn_client-v0.85.1) - 2023-06-06

### Added
- refactor replication flow to using pull model
