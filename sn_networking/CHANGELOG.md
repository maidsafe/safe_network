# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.8.16](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.15...sn_networking-v0.8.16) - 2023-10-06

### Other
- updated the following local packages: sn_transfers

## [0.8.15](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.14...sn_networking-v0.8.15) - 2023-10-05

### Added
- *(metrics)* display node reward balance metrics
- *(metrics)* display node record count metrics
- *(metrics)* enable process memory and cpu usage metrics

### Fixed
- *(metrics)* do not bind to localhost as it causes issues with containers

## [0.8.14](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.13...sn_networking-v0.8.14) - 2023-10-05

### Added
- feat!(cli): remove concurrency argument

### Fixed
- *(client)* remove concurrency limitations

## [0.8.13](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.12...sn_networking-v0.8.13) - 2023-10-05

### Other
- updated the following local packages: sn_transfers

## [0.8.12](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.11...sn_networking-v0.8.12) - 2023-10-05

### Added
- quorum for records get

## [0.8.11](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.10...sn_networking-v0.8.11) - 2023-10-05

### Other
- updated the following local packages: sn_transfers

## [0.8.10](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.9...sn_networking-v0.8.10) - 2023-10-04

### Other
- *(release)* sn_cli-v0.83.19/sn_client-v0.92.0/sn_registers-v0.3.0/sn_node-v0.91.18/sn_testnet-v0.2.181/sn_protocol-v0.7.9

## [0.8.9](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.8...sn_networking-v0.8.9) - 2023-10-04

### Other
- updated the following local packages: sn_transfers

## [0.8.8](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.7...sn_networking-v0.8.8) - 2023-10-03

### Other
- log status of pending_get_record

## [0.8.7](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.6...sn_networking-v0.8.7) - 2023-10-03

### Added
- immediate stop on RecordNotFound

## [0.8.6](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.5...sn_networking-v0.8.6) - 2023-10-03

### Added
- *(node)* remove failed records if write fails

## [0.8.5](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.4...sn_networking-v0.8.5) - 2023-10-02

### Other
- updated the following local packages: sn_transfers

## [0.8.4](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.3...sn_networking-v0.8.4) - 2023-10-02

### Other
- *(client)* more logs around StoreCost retrieveal

## [0.8.3](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.2...sn_networking-v0.8.3) - 2023-09-29

### Added
- replicate fetch from peer first then from network

## [0.8.2](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.1...sn_networking-v0.8.2) - 2023-09-28

### Other
- updated the following local packages: sn_transfers

## [0.8.1](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.0...sn_networking-v0.8.1) - 2023-09-27

### Added
- *(networking)* remove optional_semaphore being passed down from apps
- all records are Quorum::All once more

## [0.8.0](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.7.5...sn_networking-v0.8.0) - 2023-09-27

### Added
- deep clean sn_transfers, reduce exposition, remove dead code

## [0.7.5](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.7.4...sn_networking-v0.7.5) - 2023-09-26

### Added
- *(close group)* Change close group size to 5

## [0.7.4](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.7.3...sn_networking-v0.7.4) - 2023-09-26

### Added
- *(apis)* adding client and node APIs, as well as safenode RPC service to unsubscribe from gossipsub topics

## [0.7.3](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.7.2...sn_networking-v0.7.3) - 2023-09-25

### Other
- updated the following local packages: sn_transfers

## [0.7.2](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.7.1...sn_networking-v0.7.2) - 2023-09-25

### Other
- updated the following local packages: sn_transfers

## [0.7.1](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.7.0...sn_networking-v0.7.1) - 2023-09-22

### Added
- *(apis)* adding client and node APIs, as well as safenode RPC services to pub/sub to gossipsub topics
- *(network)* adding support for gossipsub behaviour/messaging

## [0.7.0](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.6.15...sn_networking-v0.7.0) - 2023-09-21

### Added
- rename utxo by CashNoteRedemption
- dusking DBCs

### Other
- rename Nano NanoTokens
- improve naming

## [0.6.15](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.6.14...sn_networking-v0.6.15) - 2023-09-21

### Other
- *(networking)* reduce identify log noise

## [0.6.14](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.6.13...sn_networking-v0.6.14) - 2023-09-20

### Added
- downward compatible for patch version updates

## [0.6.13](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.6.12...sn_networking-v0.6.13) - 2023-09-20

### Other
- major dep updates

## [0.6.12](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.6.11...sn_networking-v0.6.12) - 2023-09-20

### Other
- allow chunks to be Quorum::One
- *(networking)* enable caching of records (in theory)

## [0.6.11](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.6.10...sn_networking-v0.6.11) - 2023-09-19

### Other
- *(ntworking)* record changes to range of responsibility

## [0.6.10](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.6.9...sn_networking-v0.6.10) - 2023-09-19

### Other
- *(networking)* remove the quote from names as it's misleading

## [0.6.9](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.6.8...sn_networking-v0.6.9) - 2023-09-19

### Fixed
- shorter wait on verification put

## [0.6.8](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.6.7...sn_networking-v0.6.8) - 2023-09-18

### Fixed
- avoid verification too close to put; remove un-necessary wait for put

## [0.6.7](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.6.6...sn_networking-v0.6.7) - 2023-09-18

### Added
- generic transfer receipt

### Other
- add more docs

## [0.6.6](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.6.5...sn_networking-v0.6.6) - 2023-09-15

### Other
- refine log levels

## [0.6.5](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.6.4...sn_networking-v0.6.5) - 2023-09-14

### Added
- *(network)* enable custom node metrics
- *(network)* use NetworkConfig for network construction

### Other
- remove unused error variants
- *(network)* use builder pattern to construct the Network
- *(metrics)* rename feature flag and small fixes

## [0.6.4](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.6.3...sn_networking-v0.6.4) - 2023-09-13

### Added
- *(register)* paying nodes for Register storage

## [0.6.3](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.6.2...sn_networking-v0.6.3) - 2023-09-12

### Other
- *(networking)* add store cost / relevant record tests
- *(networking)* refactor record_store to have relevant records calculation separately

## [0.6.2](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.6.1...sn_networking-v0.6.2) - 2023-09-12

### Added
- *(network)* feature gate libp2p metrics
- *(network)* implement libp2p metrics

### Other
- *(docs)* add docs about network metrics
- *(metrics)* rename network metrics and remove from default features list
- *(network)* remove unwraps inside metrics server

## [0.6.1](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.6.0...sn_networking-v0.6.1) - 2023-09-12

### Added
- add tx and parent spends verification
- chunk payments using UTXOs instead of DBCs

### Other
- use updated sn_dbc

## [0.6.0](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.5.14...sn_networking-v0.6.0) - 2023-09-11

### Added
- [**breaking**] Clients add a tolerance to store cost
- [**breaking**] Nodes no longer tolerate underpaying

### Other
- *(release)* sn_cli-v0.81.29/sn_client-v0.88.16/sn_registers-v0.2.6/sn_node-v0.89.29/sn_testnet-v0.2.120/sn_protocol-v0.6.6

## [0.5.14](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.5.13...sn_networking-v0.5.14) - 2023-09-08

### Fixed
- reenable verify_store flag during put

### Other
- *(client)* refactor to have permits at network layer

## [0.5.13](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.5.12...sn_networking-v0.5.13) - 2023-09-07

### Other
- remove some unused code

## [0.5.12](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.5.11...sn_networking-v0.5.12) - 2023-09-07

### Added
- *(networking)* change storage cost formula

### Other
- remove unused transfer dep in networking
- *(networking)* added docs to store cost formula
- *(networking)* remove unused consts
- *(networking)* adjust formula

## [0.5.11](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.5.10...sn_networking-v0.5.11) - 2023-09-05

### Other
- *(release)* sn_cli-v0.81.21/sn_client-v0.88.11/sn_registers-v0.2.5/sn_node-v0.89.21/sn_testnet-v0.2.112/sn_protocol-v0.6.5

## [0.5.10](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.5.9...sn_networking-v0.5.10) - 2023-09-05

### Other
- *(network)* add logs on incoming connection
- *(store)* remove unused replication interval variable
- *(network)* move around SwarmDriver code
- *(network)* separate network constructor from the rest

## [0.5.9](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.5.8...sn_networking-v0.5.9) - 2023-09-04

### Other
- updated the following local packages: sn_protocol

## [0.5.8](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.5.7...sn_networking-v0.5.8) - 2023-09-04

### Other
- updated the following local packages: sn_protocol

## [0.5.7](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.5.6...sn_networking-v0.5.7) - 2023-09-01

### Other
- updated the following local packages: sn_transfers

## [0.5.6](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.5.5...sn_networking-v0.5.6) - 2023-09-01

### Other
- optimise getting furthest record

## [0.5.5](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.5.4...sn_networking-v0.5.5) - 2023-08-31

### Other
- updated the following local packages: sn_transfers

## [0.5.4](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.5.3...sn_networking-v0.5.4) - 2023-08-31

### Other
- updated the following local packages: sn_protocol, sn_transfers

## [0.5.3](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.5.2...sn_networking-v0.5.3) - 2023-08-31

### Added
- *(store)* implement `UnifiedRecordStore`
- *(store)* impl `RecordStore` for node and client separately

### Fixed
- *(store)* remove custom Record iterator

## [0.5.2](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.5.1...sn_networking-v0.5.2) - 2023-08-31

### Other
- some logging updates

## [0.5.1](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.5.0...sn_networking-v0.5.1) - 2023-08-31

### Added
- fetch from network during network

## [0.5.0](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.30...sn_networking-v0.5.0) - 2023-08-30

### Added
- refactor to allow greater upload parallelisation
- one transfer per data set, mapped dbcs to content addrs
- [**breaking**] pay each chunk holder direct
- feat!(protocol): gets keys with GetStoreCost
- feat!(protocol): get price and pay for each chunk individually

### Fixed
- *(tokio)* remove tokio fs
- *(network)* trigger bootstrap until we have enough peers

### Other
- *(networking)* increase FETCH_TIMEOUT to 10s
- trival clean ups
- *(deps)* bump tokio to 1.32.0
- *(client)* reduce transferoutputs cloning
- *(networking)* ensure we're always driving forward replication if pending
- increase concurrent fetches for replication data
- *(client)* error out early for invalid transfers
- *(networking)* return all GetStoreCost prices and use them
- *(node)* clarify payment errors

## [0.4.30](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.29...sn_networking-v0.4.30) - 2023-08-30

### Added
- *(networking)* dial unroutable peer

### Other
- cargo fmt and clippy

## [0.4.29](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.28...sn_networking-v0.4.29) - 2023-08-29

### Added
- *(node)* add feature flag for tcp/quic

### Fixed
- *(node)* refactoring code

## [0.4.28](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.27...sn_networking-v0.4.28) - 2023-08-24

### Other
- updated the following local packages: sn_transfers

## [0.4.27](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.26...sn_networking-v0.4.27) - 2023-08-22

### Fixed
- *(network)* reject large records before sending out to network

## [0.4.26](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.25...sn_networking-v0.4.26) - 2023-08-22

### Fixed
- fixes to allow upload file works properly

## [0.4.25](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.24...sn_networking-v0.4.25) - 2023-08-21

### Fixed
- *(replication)* set distance range on close group change

### Other
- *(network)* remove unused `NetworkEvent::CloseGroupUpdated`

## [0.4.24](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.23...sn_networking-v0.4.24) - 2023-08-21

### Other
- update circular vec to handle errors.

## [0.4.23](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.22...sn_networking-v0.4.23) - 2023-08-18

### Added
- remove client and node initial join flow
- *(network)* perform `kad bootstrap` from the network layer

## [0.4.22](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.21...sn_networking-v0.4.22) - 2023-08-18

### Other
- updated the following local packages: sn_protocol

## [0.4.21](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.20...sn_networking-v0.4.21) - 2023-08-17

### Fixed
- manual impl Debug for NetworkEvent

## [0.4.20](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.19...sn_networking-v0.4.20) - 2023-08-17

### Fixed
- *(client)* use boostrap and fire Connecting event

## [0.4.19](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.18...sn_networking-v0.4.19) - 2023-08-17

### Fixed
- correct calculation of is_in_close_range
- avoid download bench result polluted

## [0.4.18](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.17...sn_networking-v0.4.18) - 2023-08-15

### Fixed
- using proper distance range for filtering

## [0.4.17](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.16...sn_networking-v0.4.17) - 2023-08-11

### Added
- *(networking)* add test for any_cost_will_do
- *(networking)* enable returning less than majority for store_cost

### Fixed
- *(client)* only_store_cost_if_higher missing else added
- correct the storage_cost stepping calculation

### Other
- improve NetworkEvent logging
- *(networking)* remove logs, fix typos and clippy issues

## [0.4.16](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.15...sn_networking-v0.4.16) - 2023-08-10

### Fixed
- *(test)* have multiple verification attempts

## [0.4.15](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.14...sn_networking-v0.4.15) - 2023-08-10

### Other
- tweak the storage cost curve

## [0.4.14](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.13...sn_networking-v0.4.14) - 2023-08-08

### Added
- *(networking)* remove sign over store cost
- *(networking)* take prices[majority_index] price to avoid node quote validation
- *(transfers)* add get largest dbc for spending

### Fixed
- *(node)* prevent panic in storage calcs

### Other
- tidy store cost code

## [0.4.13](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.12...sn_networking-v0.4.13) - 2023-08-07

### Other
- record store pruning test

## [0.4.12](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.11...sn_networking-v0.4.12) - 2023-08-07

### Other
- updated the following local packages: sn_protocol, sn_transfers

## [0.4.11](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.10...sn_networking-v0.4.11) - 2023-08-04

### Added
- only fetch close enough data during Replication

## [0.4.10](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.9...sn_networking-v0.4.10) - 2023-08-03

### Other
- *(node)* NetworkEvent logs

## [0.4.9](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.8...sn_networking-v0.4.9) - 2023-08-03

### Other
- *(node)* remove peer_connected altogether during NodeEvent handler

## [0.4.8](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.7...sn_networking-v0.4.8) - 2023-08-02

### Other
- more places to log RecordKey in pretty format

## [0.4.7](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.6...sn_networking-v0.4.7) - 2023-08-01

### Other
- *(networking)* improve data pruning
- fix record store test to only return with update
- make store_cost calc stepped, and use relevant records only
- *(networking)* one in one out for data at capacity.
- *(networking)* only remove data as a last resort
- *(networking)* use TOTAL_SUPPLY from sn_transfers

## [0.4.6](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.5...sn_networking-v0.4.6) - 2023-08-01

### Other
- updated the following local packages: sn_protocol

## [0.4.5](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.4...sn_networking-v0.4.5) - 2023-08-01

### Other
- fix double spend and remove arbitrary wait
- *(release)* sn_cli-v0.80.17/sn_client-v0.87.0/sn_registers-v0.2.0/sn_node-v0.88.6/sn_testnet-v0.2.44/sn_protocol-v0.4.2

## [0.4.4](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.3...sn_networking-v0.4.4) - 2023-07-31

### Fixed
- *(test)* fix failing unit test
- *(replication)* state should progress even if MAX_PARALLEL_FETCHES is reached

### Other
- *(replication)* add unit tests

## [0.4.3](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.2...sn_networking-v0.4.3) - 2023-07-31

### Added
- carry out get_record re-attempts for critical record
- for put_record verification, NotEnoughCopies is acceptable
- cover the Kademlia completion of get_record
- resolve get_record split results
- accumulate get_record_ok to return with majority

### Other
- move PrettyPrintRecordKey to sn_protocol
- fix typo
- small refactors for failing CI

## [0.4.2](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.1...sn_networking-v0.4.2) - 2023-07-31

### Added
- *(node)* add marker for a network connection timeout

## [0.4.1](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.4.0...sn_networking-v0.4.1) - 2023-07-28

### Fixed
- *(replication)* fix incorrect fetch timeout condition

## [0.4.0](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.34...sn_networking-v0.4.0) - 2023-07-28

### Added
- *(protocol)* Add GetStoreCost Query and QueryResponse

### Other
- remove duplicate the thes

## [0.3.34](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.33...sn_networking-v0.3.34) - 2023-07-28

### Added
- retries in put records
- actionable record key errors

### Fixed
- prettier logs

### Other
- adapt all logging to use pretty record key

## [0.3.33](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.32...sn_networking-v0.3.33) - 2023-07-27

### Fixed
- *(network)* close group should only contain CLOSE_GROUP_SIZE elements
- *(node)* set distance range to prune records

## [0.3.32](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.31...sn_networking-v0.3.32) - 2023-07-26

### Other
- updated the following local packages: sn_protocol

## [0.3.31](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.30...sn_networking-v0.3.31) - 2023-07-26

### Added
- *(networking)* add in a basic store cost calculation based on record_store capacity

### Other
- *(networking)* increase verification attempts for PUT records

## [0.3.30](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.29...sn_networking-v0.3.30) - 2023-07-26

### Added
- *(networking)* record store prunes more frequently.

## [0.3.29](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.28...sn_networking-v0.3.29) - 2023-07-25

### Added
- *(replication)* replicate when our close group changes

### Fixed
- *(replication)* send out keys for replication if not empty

### Other
- *(logs)* log PeerId when a message is received

## [0.3.28](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.27...sn_networking-v0.3.28) - 2023-07-21

### Other
- updated the following local packages: sn_protocol

## [0.3.27](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.26...sn_networking-v0.3.27) - 2023-07-20

### Other
- cleanup error types

## [0.3.26](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.25...sn_networking-v0.3.26) - 2023-07-19

### Other
- updated the following local packages: sn_protocol

## [0.3.25](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.24...sn_networking-v0.3.25) - 2023-07-19

### Other
- updated the following local packages: sn_protocol

## [0.3.24](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.23...sn_networking-v0.3.24) - 2023-07-18

### Other
- *(networking)* only log queries we started
- *(networking)* remove some uneeded async

## [0.3.23](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.22...sn_networking-v0.3.23) - 2023-07-18

### Added
- *(networking)* remove LostRecordEvent

## [0.3.22](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.21...sn_networking-v0.3.22) - 2023-07-18

### Other
- *(networking)* improve connected peers count log

## [0.3.21](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.20...sn_networking-v0.3.21) - 2023-07-17

### Fixed
- *(sn_networking)* revert multiaddr pop fn

## [0.3.20](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.19...sn_networking-v0.3.20) - 2023-07-17

### Added
- *(networking)* drop network events if channel is full
- *(networking)* upgrade to libp2p 0.52.0

### Other
- *(networking)* log all connected peer count

## [0.3.19](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.18...sn_networking-v0.3.19) - 2023-07-12

### Other
- updated the following local packages: sn_protocol

## [0.3.18](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.17...sn_networking-v0.3.18) - 2023-07-11

### Fixed
- prevent multiple concurrent get_closest calls when joining

## [0.3.17](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.16...sn_networking-v0.3.17) - 2023-07-11

### Other
- updated the following local packages: sn_protocol

## [0.3.16](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.15...sn_networking-v0.3.16) - 2023-07-11

### Added
- *(node)* shuffle data waiting for fetch

### Other
- *(node)* only log LostRecord when peersfound

## [0.3.15](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.14...sn_networking-v0.3.15) - 2023-07-10

### Added
- *(node)* remove any data we have from replication queue

### Other
- *(node)* cleanup unused SwarmCmd for GetAllRecordAddrs

## [0.3.14](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.13...sn_networking-v0.3.14) - 2023-07-10

### Added
- client upload Register via put_record

## [0.3.13](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.12...sn_networking-v0.3.13) - 2023-07-06

### Other
- add docs to `dialed_peers` for explanation

## [0.3.12](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.11...sn_networking-v0.3.12) - 2023-07-06

### Added
- PutRecord response during client upload
- client upload chunk using kad::put_record

### Other
- small tidy up

## [0.3.11](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.10...sn_networking-v0.3.11) - 2023-07-06

### Other
- updated the following local packages: sn_logging

## [0.3.10](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.9...sn_networking-v0.3.10) - 2023-07-05

### Added
- disable record filter; send duplicated record to validation for doube spend detection
- carry out validation for record_store::put

## [0.3.9](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.8...sn_networking-v0.3.9) - 2023-07-05

### Other
- updated the following local packages: sn_protocol

## [0.3.8](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.7...sn_networking-v0.3.8) - 2023-07-04

### Other
- remove dirs-next dependency

## [0.3.7](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.6...sn_networking-v0.3.7) - 2023-07-04

### Other
- updated the following local packages: sn_protocol

## [0.3.6](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.5...sn_networking-v0.3.6) - 2023-07-03

### Fixed
- avoid duplicated replications

## [0.3.5](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.4...sn_networking-v0.3.5) - 2023-06-29

### Added
- *(node)* write secret key to disk and re-use

## [0.3.4](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.3...sn_networking-v0.3.4) - 2023-06-28

### Added
- *(node)* add missing send_event calls
- *(node)* non blocking channels

## [0.3.3](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.2...sn_networking-v0.3.3) - 2023-06-28

### Other
- updated the following local packages: sn_protocol

## [0.3.2](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.1...sn_networking-v0.3.2) - 2023-06-28

### Fixed
- *(networking)* local-discovery should not be default

## [0.3.1](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.3.0...sn_networking-v0.3.1) - 2023-06-28

### Added
- *(node)* dial without PeerId

## [0.3.0](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.2.3...sn_networking-v0.3.0) - 2023-06-27

### Added
- append peer id to node's default root dir

## [0.2.3](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.2.2...sn_networking-v0.2.3) - 2023-06-27

### Other
- *(networking)* make some errors log properly

## [0.2.2](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.2.1...sn_networking-v0.2.2) - 2023-06-26

### Fixed
- get_closest_local shall only return CLOSE_GROUP_SIZE peers

## [0.2.1](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.2.0...sn_networking-v0.2.1) - 2023-06-26

### Other
- Revert "feat: append peer id to node's default root dir"

## [0.2.0](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.24...sn_networking-v0.2.0) - 2023-06-26

### Added
- append peer id to node's default root dir

## [0.1.24](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.23...sn_networking-v0.1.24) - 2023-06-26

### Other
- updated the following local packages: sn_logging

## [0.1.23](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.22...sn_networking-v0.1.23) - 2023-06-24

### Other
- log detailed peer distance and kBucketTable stats

## [0.1.22](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.21...sn_networking-v0.1.22) - 2023-06-23

### Other
- *(networking)* reduce some log levels to make 'info' more useful

## [0.1.21](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.20...sn_networking-v0.1.21) - 2023-06-23

### Added
- repliate to peers lost record

## [0.1.20](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.19...sn_networking-v0.1.20) - 2023-06-23

### Added
- *(node)* only add to routing table after Identify success

## [0.1.19](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.18...sn_networking-v0.1.19) - 2023-06-22

### Fixed
- improve client upload speed

## [0.1.18](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.17...sn_networking-v0.1.18) - 2023-06-21

### Added
- *(node)* trigger replication when inactivity

## [0.1.17](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.16...sn_networking-v0.1.17) - 2023-06-21

### Other
- *(network)* remove `NetworkEvent::PutRecord` dead code

## [0.1.16](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.15...sn_networking-v0.1.16) - 2023-06-21

### Other
- updated the following local packages: sn_protocol

## [0.1.15](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.14...sn_networking-v0.1.15) - 2023-06-21

### Other
- updated the following local packages: sn_logging

## [0.1.14](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.13...sn_networking-v0.1.14) - 2023-06-20

### Added
- *(network)* validate `Record` on GET
- *(network)* validate and store `ReplicatedData`
- *(node)* perform proper validations on PUT
- *(network)* validate and store `Record`
- *(kad)* impl `RecordHeader` to store the record kind

### Fixed
- *(network)* use `rmp_serde` for `RecordHeader` ser/de
- *(network)* Send `Request` without awaiting for `Response`

### Other
- *(docs)* add more docs and comments

## [0.1.13](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.12...sn_networking-v0.1.13) - 2023-06-20

### Added
- *(sn_networking)* Make it possible to pass in a keypair for PeerID

## [0.1.12](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.11...sn_networking-v0.1.12) - 2023-06-20

### Other
- updated the following local packages: sn_protocol

## [0.1.11](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.10...sn_networking-v0.1.11) - 2023-06-20

### Other
- reduce some log levels to make 'debug' more useful

## [0.1.10](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.9...sn_networking-v0.1.10) - 2023-06-15

### Fixed
- parent spend checks
- parent spend issue

## [0.1.9](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.8...sn_networking-v0.1.9) - 2023-06-14

### Added
- include output DBC within payment proof for Chunks storage

## [0.1.8](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.7...sn_networking-v0.1.8) - 2023-06-14

### Added
- prune out of range record entries

## [0.1.7](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.6...sn_networking-v0.1.7) - 2023-06-14

### Added
- *(client)* increase default request timeout
- *(client)* expose req/resp timeout to client cli

### Other
- *(networking)* update naming of REQUEST_TIMEOUT_DEFAULT_S

## [0.1.6](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.5...sn_networking-v0.1.6) - 2023-06-13

### Other
- updated the following local packages: sn_logging

## [0.1.5](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.4...sn_networking-v0.1.5) - 2023-06-12

### Added
- remove spendbook rw locks, improve logging

## [0.1.4](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.3...sn_networking-v0.1.4) - 2023-06-12

### Other
- updated the following local packages: sn_record_store

## [0.1.3](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.1.2...sn_networking-v0.1.3) - 2023-06-09

### Other
- manually change crate version
- heavier load during the churning test
- *(client)* trival log improvement
- Revert "chore(release): sn_cli-v0.77.1/sn_client-v0.85.2/sn_networking-v0.1.2/sn_node-v0.83.1"

## [0.1.1](https://github.com/jacderida/safe_network/compare/sn_networking-v0.1.0...sn_networking-v0.1.1) - 2023-06-06

### Added
- refactor replication flow to using pull model
- *(node)* remove delay for Identify

### Other
- *(node)* return proper error if failing to create storage dir

## [0.1.0](https://github.com/jacderida/safe_network/releases/tag/sn_networking-v0.1.0) - 2023-06-04

### Added
- record based DBC Spends
- *(record_store)* extract record_store into its own crate

### Fixed
- expand channel capacity
- *(node)* correct dead peer detection
- *(node)* increase replication range to 5.
- add in init to potential_dead_peers.
- remove unused deps after crate reorg
- *(networking)* clippy
- local-discovery deps
- remove unused deps, fix doc comment

### Other
- increase networking channel size
- *(CI)* mem check against large file and churn test
- fixup after rebase
- extract logging and networking crates
