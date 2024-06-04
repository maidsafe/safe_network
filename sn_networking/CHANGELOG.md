# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.16.5](https://github.com/joshuef/safe_network/compare/sn_networking-v0.16.4...sn_networking-v0.16.5) - 2024-06-04

### Other
- release
- release
- *(release)* sn_client-v0.107.5/sn_networking-v0.16.3/sn_cli-v0.93.4/sn_node-v0.107.4/node-launchpad-v0.3.5/sn-node-manager-v0.9.4/sn_auditor-v0.1.23/sn_peers_acquisition-v0.3.3/sn_faucet-v0.4.25/sn_node_rpc_client-v0.6.22
- *(network)* set metrics server to run on localhost

## [0.16.4](https://github.com/joshuef/safe_network/compare/sn_networking-v0.16.3...sn_networking-v0.16.4) - 2024-06-04

### Other
- updated the following local packages: sn_transfers

## [0.16.3](https://github.com/joshuef/safe_network/compare/sn_networking-v0.16.2...sn_networking-v0.16.3) - 2024-06-04

### Other
- *(network)* set metrics server to run on localhost

## [0.16.2](https://github.com/joshuef/safe_network/compare/sn_networking-v0.16.1...sn_networking-v0.16.2) - 2024-06-03

### Other
- updated the following local packages: sn_transfers

## [0.16.1](https://github.com/joshuef/safe_network/compare/sn_networking-v0.16.0...sn_networking-v0.16.1) - 2024-06-03

### Other
- bump versions to enable re-release with env vars at compilation

## [0.16.0](https://github.com/joshuef/safe_network/compare/sn_networking-v0.15.3...sn_networking-v0.16.0) - 2024-06-03

### Added
- *(networking)* add UPnP metrics
- *(network)* [**breaking**] move network versioning away from sn_protocol

### Fixed
- *(networking)* upnp feature gates for metrics
- *(networking)* conditional upnp metrics

### Other
- *(networking)* cargo fmt

## [0.15.3](https://github.com/joshuef/safe_network/compare/sn_networking-v0.15.2...sn_networking-v0.15.3) - 2024-05-24

### Added
- *(metrics)* expose store cost value
- keep track of the estimated network size metric
- record lip2p relay and dctur metrics
- *(node)* periodically forward reward to specific address

### Fixed
- avoid adding mixed type addresses into RT
- enable libp2p metrics to be captured

### Other
- *(node)* tuning the pricing curve
- *(node)* remove un-necessary is_relayed check inside add_potential_candidates
- move historic_quoting_metrics out of the record_store dir
- clippy fixes for open metrics feature
- make open metrics feature default but without starting it by default
- *(networking)* update tests for pricing curve tweaks
- *(refactor)* stabilise node size to 4k records,
- Revert "feat(node): make spend and cash_note reason field configurable"
- Revert "chore: rename output reason to purpose for clarity"

## [0.15.2](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.15.1...sn_networking-v0.15.2) - 2024-05-09

### Fixed
- *(relay_manager)* filter out bad nodes

## [0.15.1](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.15.0...sn_networking-v0.15.1) - 2024-05-08

### Other
- *(release)* sn_registers-v0.3.13

## [0.15.0-alpha.6](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.15.0-alpha.5...sn_networking-v0.15.0-alpha.6) - 2024-05-07

### Added
- *(network)* add --upnp flag to node
- *(networking)* feature gate 'upnp'
- *(networking)* add UPnP behavior to open port
- *(node)* make spend and cash_note reason field configurable
- *(relay)* remove autonat and enable hole punching manually
- *(relay)* remove old listen addr if we are using a relayed connection
- *(relay)* update the relay manager if the listen addr has been closed
- *(relay)* remove the dial flow
- *(relay)* impl RelayManager to perform circuit relay when behind NAT
- *(networking)* add in autonat server basics
- *(neetworking)* initial tcp use by default
- *(networking)* clear  record on valid put
- *(node)* restrict replication fetch range when node is full
- *(store)* load existing records in parallel
- [**breaking**] renamings in CashNote
- *(node)* notify peer it is now considered as BAD
- *(node)* restore historic quoting metrics to allow restart
- *(networking)* shift to use ilog2 bucket distance for close data calcs
- report protocol mismatch error

### Fixed
- *(networking)* allow wasm32 compilation
- *(network)* remove all external addresses related to a relay server
- *(relay_manager)* remove external addr on connection close
- relay server should not close connections made to a reserved peer
- short circuit identify if the peer is already present in the routitng table
- update outdated connection removal flow
- do not remove outdated connections
- increase relay server capacity
- keep idle connections forever
- pass peer id while crafting relay address
- *(relay)* crafted multi address should contain the P2PCircuit protocol
- do not add reported external addressese if we are behind home network
- *(networking)* do not add to dialed peers
- *(network)* do not strip out relay's PeerId
- *(relay)* craft the correctly formatted relay address
- *(network)* do not perform AutoNat for clients
- *(relay_manager)* do not dial with P2PCircuit protocol
- *(test)* quoting metrics might have live_time field changed along time
- *(node)* avoid false alert on FailedLocalRecord
- *(record_store)* prune only one record at a time
- *(node)* notify replication_fetcher of early completion
- *(node)* fetcher completes on_going_fetch entry on record_key only
- *(node)* not send out replication when failed read from local
- *(networking)* increase the local responsible range of nodes to K_VALUE peers away
- *(network)* clients should not perform farthest relevant record check
- *(node)* replication_fetch keep distance_range sync with record_store
- *(node)* replication_list in range filter

### Other
- *(versions)* sync versions with latest crates.io vs
- cargo fmt
- rename output reason to purpose for clarity
- store owner info inside node instead of network
- *(network)* move event handling to its own module
- cleanup network events
- *(network)* remove nat detection via incoming connections check
- enable connection keepalive timeout
- remove non relayed listener id from relay manager
- enable multiple relay connections
- return early if peer is not a node
- *(tryout)* do not add new relay candidates
- add debug lines while adding potential relay candidates
- do not remove old non-relayed listeners
- clippy fix
- *(networking)* remove empty file
- *(networking)* re-add global_only
- use quic again
- log listner id
- *(relay)* add candidate even if we are dialing
- remove quic
- cleanup, add in relay server behaviour, and todo
- *(node)* lower some log levels to reduce log size
- *(node)* optimise record_store farthest record calculation
- *(node)* do not reset farthest_acceptance_distance
- *(node)* remove duplicated record_store fullness check
- *(networking)* notify network event on failed put due to prune
- *(networking)* ensure pruned data is indeed further away than kept
- *(CI)* confirm there is no failed replication fetch
- *(networking)* remove circular vec error
- *(node)* unit test for recover historic quoting metrics
- *(deps)* bump dependencies
- *(node)* pass entire QuotingMetrics into calculate_cost_for_records
- *(node)* extend distance range

## [0.14.1](https://github.com/joshuef/safe_network/compare/sn_networking-v0.14.0...sn_networking-v0.14.1) - 2024-03-28

### Other
- updated the following local packages: sn_transfers

## [0.14.0](https://github.com/joshuef/safe_network/compare/sn_networking-v0.13.35...sn_networking-v0.14.0) - 2024-03-27

### Added
- *(networking)* add NodeIssue for tracking bad node shunning
- [**breaking**] remove gossip code
- *(network)* filter out peers when returning store cost
- use Arc inside Client, Network to reduce clone cost

### Fixed
- *(node)* fetching new data shall not cause timed_out immediately
- *(test)* generate unique temp dir to avoid read outdated data

### Other
- *(node)* refactor pricing metrics
- lower some networking log levels
- *(node)* loose bad node detection criteria
- *(node)* optimization to reduce logging

## [0.13.35](https://github.com/joshuef/safe_network/compare/sn_networking-v0.13.34...sn_networking-v0.13.35) - 2024-03-21

### Added
- dag error recording

### Other
- *(node)* reduce bad_nodes check resource usage

## [0.13.34](https://github.com/joshuef/safe_network/compare/sn_networking-v0.13.33...sn_networking-v0.13.34) - 2024-03-18

### Added
- *(networking)* listen on WS addr too
- *(networking)* support fallback WS transport

## [0.13.33](https://github.com/joshuef/safe_network/compare/sn_networking-v0.13.32...sn_networking-v0.13.33) - 2024-03-14

### Added
- refactor spend validation

### Fixed
- *(test)* use unqiue dir during test
- dont stop spend verification at spend error, generalise spend serde
- put validation network spends errors management

### Other
- improve code quality
- *(release)* sn_transfers-v0.16.3/sn_cli-v0.89.82

## [0.13.32](https://github.com/joshuef/safe_network/compare/sn_networking-v0.13.31-alpha.0...sn_networking-v0.13.32) - 2024-03-08

### Other
- updated the following local packages: sn_transfers

## [0.13.30](https://github.com/joshuef/safe_network/compare/sn_networking-v0.13.29...sn_networking-v0.13.30) - 2024-03-06

### Added
- *(node)* exponential pricing when storage reaches high
- *(node)* bad verification to exclude connections from bad_nodes
- collect royalties through DAG
- *(node)* record_store chunk in batch and setup distance_range

### Fixed
- filter out spent cashnotes in received client transfers
- record_store no longer update distance_range via close_group change

### Other
- clean swarm commands errs and spend errors
- *(release)* sn_transfers-v0.16.1
- *(release)* sn_protocol-v0.15.0/sn-node-manager-v0.4.0

## [0.13.29](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.28...sn_networking-v0.13.29) - 2024-02-23

### Added
- *(node)* error out bad_nodes to node via event channel
- *(node)* refactor replication_fetcher to black list bad nodes

## [0.13.28](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.27...sn_networking-v0.13.28) - 2024-02-21

### Other
- updated the following local packages: sn_protocol

## [0.13.27](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.26...sn_networking-v0.13.27) - 2024-02-20

### Other
- updated the following local packages: sn_protocol

## [0.13.26](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.25...sn_networking-v0.13.26) - 2024-02-20

### Added
- *(node)* fetch new data copy immediately

## [0.13.25](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.24...sn_networking-v0.13.25) - 2024-02-20

### Added
- *(networking)* on start, record_store repopulates from existing

### Other
- *(networking)* add logs for preexisting record loading

## [0.13.24](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.23...sn_networking-v0.13.24) - 2024-02-20

### Other
- *(release)* sn_client-v0.104.20/sn_registers-v0.3.10/sn_node-v0.104.28/sn_cli-v0.89.73/sn_protocol-v0.14.3/sn_faucet-v0.3.72/sn_node_rpc_client-v0.4.59

## [0.13.23](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.22...sn_networking-v0.13.23) - 2024-02-19

### Added
- *(node)* terminate node on too many HDD write errors

## [0.13.22](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.21...sn_networking-v0.13.22) - 2024-02-19

### Other
- *(client)* handle kad event put_record result

## [0.13.21](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.20...sn_networking-v0.13.21) - 2024-02-19

### Added
- *(networking)* remove all pending replication from failed nodes

### Other
- *(networking)* update the replication fetcher tests, now we cleanup failed nodes

## [0.13.20](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.19...sn_networking-v0.13.20) - 2024-02-15

### Other
- updated the following local packages: sn_transfers

## [0.13.19](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.18...sn_networking-v0.13.19) - 2024-02-15

### Added
- *(networking)* log only unconfirmed ext. addrs
- *(networking)* add candidate addr as external

### Fixed
- *(networking)* no external addr if client

## [0.13.18](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.17...sn_networking-v0.13.18) - 2024-02-15

### Other
- updated the following local packages: sn_protocol

## [0.13.17](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.16...sn_networking-v0.13.17) - 2024-02-14

### Other
- updated the following local packages: sn_protocol

## [0.13.16](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.15...sn_networking-v0.13.16) - 2024-02-14

### Other
- updated the following local packages: sn_protocol, sn_transfers

## [0.13.15](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.14...sn_networking-v0.13.15) - 2024-02-13

### Other
- updated the following local packages: sn_protocol

## [0.13.14](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.13...sn_networking-v0.13.14) - 2024-02-13

### Other
- updated the following local packages: sn_transfers

## [0.13.13](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.12...sn_networking-v0.13.13) - 2024-02-12

### Other
- *(networking)* clear all stats afgter we log them
- *(networking)* improve swarm driver stats logging

## [0.13.12](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.11...sn_networking-v0.13.12) - 2024-02-12

### Other
- *(node)* optimize Cmd::Replicate handling flow

## [0.13.11](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.10...sn_networking-v0.13.11) - 2024-02-09

### Fixed
- *(node)* store records even with max_records reached

## [0.13.10](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.9...sn_networking-v0.13.10) - 2024-02-09

### Other
- *(node)* disable metrics record

## [0.13.9](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.8...sn_networking-v0.13.9) - 2024-02-08

### Other
- copyright update to current year

## [0.13.8](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.7...sn_networking-v0.13.8) - 2024-02-08

### Added
- move the RetryStrategy into protocol and use that during cli upload/download
- *(network)* impl RetryStrategy to make the reattempts flexible

### Other
- *(network)* rename re-attempts to retry strategy

## [0.13.7](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.6...sn_networking-v0.13.7) - 2024-02-08

### Added
- *(networking)* remove AutoNAT

### Fixed
- *(networking)* solve large_enum_variant warning

## [0.13.6](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.5...sn_networking-v0.13.6) - 2024-02-07

### Other
- updated the following local packages: sn_transfers

## [0.13.5](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.4...sn_networking-v0.13.5) - 2024-02-06

### Other
- updated the following local packages: sn_transfers

## [0.13.4](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.3...sn_networking-v0.13.4) - 2024-02-05

### Fixed
- *(node)* avoid logging record value

## [0.13.3](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.2...sn_networking-v0.13.3) - 2024-02-05

### Fixed
- avoid log raw bytes of key accidently

## [0.13.2](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.1...sn_networking-v0.13.2) - 2024-02-05

### Other
- updated the following local packages: sn_protocol

## [0.13.1](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.13.0...sn_networking-v0.13.1) - 2024-02-02

### Added
- *(nodes)* make encryption of records a feature, disabled by default

## [0.13.0](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.46...sn_networking-v0.13.0) - 2024-02-02

### Other
- [**breaking**] renaming LocalWallet to HotWallet as it holds the secret key for signing tx

## [0.12.46](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.45...sn_networking-v0.12.46) - 2024-02-01

### Fixed
- *(node)* clean up on_going_fetch as well

## [0.12.45](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.44...sn_networking-v0.12.45) - 2024-02-01

### Fixed
- *(cli)* chunk manager to return error if fs operation fails

## [0.12.44](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.43...sn_networking-v0.12.44) - 2024-02-01

### Fixed
- *(network)* refactor cfg to allow get_record reattempts

## [0.12.43](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.42...sn_networking-v0.12.43) - 2024-01-31

### Fixed
- evict node on handshake timeout

## [0.12.42](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.41...sn_networking-v0.12.42) - 2024-01-30

### Added
- *(nodes)* encrypt all records before disk, decrypt on get

## [0.12.41](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.40...sn_networking-v0.12.41) - 2024-01-30

### Other
- updated the following local packages: sn_protocol

## [0.12.40](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.39...sn_networking-v0.12.40) - 2024-01-29

### Other
- updated the following local packages: sn_transfers

## [0.12.39](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.38...sn_networking-v0.12.39) - 2024-01-25

### Other
- *(test)* remove unused structs

## [0.12.38](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.37...sn_networking-v0.12.38) - 2024-01-25

### Added
- client webtransport-websys feat

### Other
- use a single target_arch.rs to simplify imports for wasm32 or no

## [0.12.37](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.36...sn_networking-v0.12.37) - 2024-01-24

### Other
- *(test)* lift up the expectations within address sim test

## [0.12.36](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.35...sn_networking-v0.12.36) - 2024-01-24

### Added
- client webtransport-websys feat
- initial webtransport-websys wasm setup

### Fixed
- *(node)* warn if "(deleted)" exists in exe name during restart

### Other
- tidy up wasm32 as target arch rather than a feat

## [0.12.35](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.34...sn_networking-v0.12.35) - 2024-01-22

### Other
- updated the following local packages: sn_protocol

## [0.12.34](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.33...sn_networking-v0.12.34) - 2024-01-22

### Other
- updated the following local packages: sn_transfers

## [0.12.33](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.32...sn_networking-v0.12.33) - 2024-01-18

### Other
- updated the following local packages: sn_protocol

## [0.12.32](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.31...sn_networking-v0.12.32) - 2024-01-18

### Added
- set quic as default transport

## [0.12.31](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.30...sn_networking-v0.12.31) - 2024-01-18

### Other
- updated the following local packages: sn_transfers

## [0.12.30](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.29...sn_networking-v0.12.30) - 2024-01-16

### Other
- updated the following local packages: sn_transfers

## [0.12.29](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.28...sn_networking-v0.12.29) - 2024-01-15

### Other
- updated the following local packages: sn_protocol

## [0.12.28](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.27...sn_networking-v0.12.28) - 2024-01-15

### Other
- updated the following local packages: sn_transfers

## [0.12.27](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.26...sn_networking-v0.12.27) - 2024-01-12

### Other
- *(network)* collect swarm_driver handling time statistics

## [0.12.26](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.25...sn_networking-v0.12.26) - 2024-01-11

### Other
- *(client)* refactor client upload flow
- *(release)* sn_cli-v0.88.9/sn_client-v0.101.5/sn_registers-v0.3.7/sn_faucet-v0.2.9/sn_node-v0.102.9/sn_node_rpc_client-v0.2.9/sn_testnet-v0.3.8/sn_protocol-v0.10.6

## [0.12.25](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.24...sn_networking-v0.12.25) - 2024-01-11

### Fixed
- *(record_store)* make event sender mandatory as they perform critical tasks

### Other
- *(record_store)* emit swarm cmd directly after writing a record

## [0.12.24](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.23...sn_networking-v0.12.24) - 2024-01-10

### Other
- updated the following local packages: sn_transfers

## [0.12.23](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.22...sn_networking-v0.12.23) - 2024-01-09

### Added
- *(client)* extra sleep between chunk verification

## [0.12.22](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.21...sn_networking-v0.12.22) - 2024-01-09

### Other
- *(node)* move add_to_replicate_fetcher to driver
- *(node)* move replication cmd flow to swarm_driver
- get spend from network only require Majority

## [0.12.21](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.20...sn_networking-v0.12.21) - 2024-01-08

### Other
- *(node)* simplify GetStoreCost flow

## [0.12.20](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.19...sn_networking-v0.12.20) - 2024-01-08

### Other
- updated the following local packages: sn_transfers

## [0.12.19](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.18...sn_networking-v0.12.19) - 2024-01-08

### Other
- *(CI)* loose the address_distribution_sim test

## [0.12.18](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.17...sn_networking-v0.12.18) - 2024-01-05

### Other
- updated the following local packages: sn_protocol, sn_transfers

## [0.12.17](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.16...sn_networking-v0.12.17) - 2024-01-05

### Added
- *(network)* move the kad::put_record_to inside PutRecordCfg

## [0.12.16](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.15...sn_networking-v0.12.16) - 2024-01-03

### Other
- no more max_records cap

## [0.12.15](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.14...sn_networking-v0.12.15) - 2024-01-02

### Added
- pick cheapest payee using linear pricing curve

## [0.12.14](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.13...sn_networking-v0.12.14) - 2023-12-29

### Added
- *(networking)* remove problematic peers from routing table

## [0.12.13](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.12...sn_networking-v0.12.13) - 2023-12-29

### Added
- use put_record_to during upload chunk

## [0.12.12](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.11...sn_networking-v0.12.12) - 2023-12-26

### Other
- *(logs)* annotate selected messages and log at info level for vdash

## [0.12.11](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.10...sn_networking-v0.12.11) - 2023-12-22

### Other
- address distribution sim

## [0.12.10](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.9...sn_networking-v0.12.10) - 2023-12-19

### Added
- network royalties through audit POC

## [0.12.9](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.8...sn_networking-v0.12.9) - 2023-12-19

### Added
- random select payee

## [0.12.8](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.7...sn_networking-v0.12.8) - 2023-12-19

### Fixed
- no retry_after to avoid looping

## [0.12.7](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.6...sn_networking-v0.12.7) - 2023-12-18

### Other
- updated the following local packages: sn_transfers

## [0.12.6](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.5...sn_networking-v0.12.6) - 2023-12-14

### Other
- *(protocol)* print the first six hex characters for every address type

## [0.12.5](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.4...sn_networking-v0.12.5) - 2023-12-14

### Added
- *(networking)* add backoff to PUT retries
- *(networking)* use backoff for get_record

## [0.12.4](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.3...sn_networking-v0.12.4) - 2023-12-14

### Fixed
- *(network)* return a map of responses instead of a vec
- *(network)* remove unused error and don't mask get record errors
- *(network)* get quourum value fn

### Other
- *(network)* return error with more info during quorum failure
- *(network)* use the entry API instead of remove and insert

## [0.12.3](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.2...sn_networking-v0.12.3) - 2023-12-14

### Other
- *(networking)* increase min verification wait to 300ms

## [0.12.2](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.1...sn_networking-v0.12.2) - 2023-12-13

### Other
- *(networking)* include record count and max records in logfile output

## [0.12.1](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.12.0...sn_networking-v0.12.1) - 2023-12-12

### Other
- updated the following local packages: sn_protocol

## [0.12.0](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.11.10...sn_networking-v0.12.0) - 2023-12-12

### Added
- *(networking)* sort quotes by closest NetworkAddress before truncate
- *(networking)* add flow to mark record as stored post-write
- *(networking)* do not return record if still being written
- *(node)* try and replicate already existing records to neighbours

### Fixed
- *(networking)* return Vec for closest queries to reliably sort

### Other
- dont log all keys during replication
- *(networking)* add replication logs
- minor updates to naming for clarity of KeysToFetchForReplication
- *(networking)* solidify REPLICATION_RANGE use. exclude self_peer_id in some calcs

## [0.11.10](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.11.9...sn_networking-v0.11.10) - 2023-12-11

### Added
- close outdated connections to non-RT peers

### Other
- gossipsub flood_publish and longer cache time to avoid loop

## [0.11.9](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.11.8...sn_networking-v0.11.9) - 2023-12-07

### Fixed
- *(network)* implement custom Debug for GetRecordError

## [0.11.8](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.11.7...sn_networking-v0.11.8) - 2023-12-06

### Other
- *(network)* use PUT Quorum::One for chunks
- *(network)* add docs for PUT Quorum
- *(network)* move the retry attempt check to a single one
- *(network)* add more docs to the get_record_handlers
- *(network)* remove custom early completion for chunks
- *(network)* check for target record during kad event handling
- *(network)* keep the GetRecordCfg inside the SwarmDriver
- *(network)* move get_record code to its own file

## [0.11.7](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.11.6...sn_networking-v0.11.7) - 2023-12-06

### Added
- replace bootstrap node if bucket full

## [0.11.6](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.11.5...sn_networking-v0.11.6) - 2023-12-06

### Other
- updated the following local packages: sn_transfers

## [0.11.5](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.11.4...sn_networking-v0.11.5) - 2023-12-06

### Other
- remove some needless cloning
- remove needless pass by value
- use inline format args
- add boilerplate for workspace lints

## [0.11.4](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.11.3...sn_networking-v0.11.4) - 2023-12-05

### Added
- *(network)* use custom enum for get_record errors

### Fixed
- *(node)* get self spend should be aggregated even if it errors out

### Other
- *(network)* use HashMap entry to insert peer into the result_map
- *(network)* avoid losing error info by converting them to a single type

## [0.11.3](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.11.2...sn_networking-v0.11.3) - 2023-12-05

### Other
- updated the following local packages: sn_transfers

## [0.11.2](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.11.1...sn_networking-v0.11.2) - 2023-12-05

### Added
- not dial back for peers in full kbucket
- *(network)* dial back when received identify from incoming

## [0.11.1](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.11.0...sn_networking-v0.11.1) - 2023-12-05

### Other
- *(node)* refactor NetworkEvent handling
- *(network)* allow replication even below K_VALUE peers
- *(networking)* dont resort closest peers list
- tie node reward test to number of data.
- *(networking)* remove triggered bootstrap slowdown
- *(networking)* remove extended spend wait before verification
- log swarm.NetworkInfo
- log on query

## [0.11.0](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.27...sn_networking-v0.11.0) - 2023-12-01

### Added
- *(network)* use seperate PUT/GET configs

### Other
- *(ci)* fix CI build cache parsing error
- *(network)* [**breaking**] use the Quorum struct provided by libp2p

## [0.10.27](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.26...sn_networking-v0.10.27) - 2023-11-29

### Added
- *(node)* only parse replication list from close peers.

## [0.10.26](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.25...sn_networking-v0.10.26) - 2023-11-29

### Other
- logging identify ops more accurately

## [0.10.25](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.24...sn_networking-v0.10.25) - 2023-11-29

### Added
- *(networking)* more properly handle outgoing errors

## [0.10.24](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.23...sn_networking-v0.10.24) - 2023-11-29

### Added
- verify spends through the cli

## [0.10.23](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.22...sn_networking-v0.10.23) - 2023-11-28

### Other
- updated the following local packages: sn_transfers

## [0.10.22](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.21...sn_networking-v0.10.22) - 2023-11-28

### Other
- updated the following local packages: sn_protocol

## [0.10.21](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.20...sn_networking-v0.10.21) - 2023-11-27

### Added
- *(discovery)* use the results of the get_closest_query
- *(discovery)* try to use random candidates from a bucket when available
- *(rpc)* return the KBuckets map

### Fixed
- *(discovery)* insert newly seen candidates and return random candidates

### Other
- changes based on comment, use btreemap
- *(discovery)* rename structs and add docs

## [0.10.20](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.19...sn_networking-v0.10.20) - 2023-11-23

### Added
- record put retry even when not verifying
- adapt retry to only when verification fails
- retry at the record level, remove all other retries, report errors
- query specific kbuckets for bootstrap

### Other
- replace bootstrap with query specific kbucket

## [0.10.19](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.18...sn_networking-v0.10.19) - 2023-11-23

### Added
- *(networking)* no floodsub publish

## [0.10.18](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.17...sn_networking-v0.10.18) - 2023-11-23

### Other
- updated the following local packages: sn_transfers

## [0.10.17](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.16...sn_networking-v0.10.17) - 2023-11-23

### Other
- *(networking)* improve logs around replication

## [0.10.16](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.15...sn_networking-v0.10.16) - 2023-11-22

### Other
- *(release)* non gossip handler shall not throw gossip msg up

## [0.10.15](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.14...sn_networking-v0.10.15) - 2023-11-21

### Added
- make joining gossip for clients and rpc nodes optional
- *(sn_networking)* no gossip for clients via Toggle

### Other
- *(sn_networking)* enable_gossip via the builder pattern
- update test setup for clients that also listen to gossip

## [0.10.14](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.13...sn_networking-v0.10.14) - 2023-11-21

### Other
- not using seen_cache when add replication list

## [0.10.13](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.12...sn_networking-v0.10.13) - 2023-11-20

### Added
- *(networking)* shortcircuit response sending for replication

### Other
- be more specific for Request matching.

## [0.10.12](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.11...sn_networking-v0.10.12) - 2023-11-20

### Other
- *(node)* set gossipsub heartbeat interval to 5secs instead of 1sec

## [0.10.11](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.10...sn_networking-v0.10.11) - 2023-11-20

### Added
- quotes

### Fixed
- use actual quote instead of dummy

## [0.10.10](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.9...sn_networking-v0.10.10) - 2023-11-17

### Other
- *(client)* increase verification delay

## [0.10.9](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.8...sn_networking-v0.10.9) - 2023-11-16

### Other
- reduce AddKeysToReplicationFetcher processing time

## [0.10.8](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.7...sn_networking-v0.10.8) - 2023-11-16

### Added
- massive cleaning to prepare for quotes

## [0.10.7](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.6...sn_networking-v0.10.7) - 2023-11-15

### Other
- updated the following local packages: sn_protocol

## [0.10.6](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.5...sn_networking-v0.10.6) - 2023-11-15

### Other
- updated the following local packages: sn_protocol, sn_transfers

## [0.10.5](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.4...sn_networking-v0.10.5) - 2023-11-14

### Other
- *(royalties)* verify royalties fees amounts

## [0.10.4](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.3...sn_networking-v0.10.4) - 2023-11-14

### Added
- *(networking)* drop excessive AddKeysToReplicationFetcher cmds

## [0.10.3](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.2...sn_networking-v0.10.3) - 2023-11-14

### Added
- dont artifically push replication

### Other
- *(networking)* calm down replication
- *(netowrking)* log incoming gossip msg ids

## [0.10.2](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.1...sn_networking-v0.10.2) - 2023-11-13

### Added
- no throwing up if not a gossip listener

## [0.10.1](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.10.0...sn_networking-v0.10.1) - 2023-11-10

### Other
- updated the following local packages: sn_transfers

## [0.10.0](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.32...sn_networking-v0.10.0) - 2023-11-10

### Added
- verify chunks with Quorum::N(2)
- *(sn_networking)* get store cost only from majority
- *(client)* only pay one node

### Fixed
- *(networking)* add put_record_once argument
- *(sn_networking)* if record already stored, 0 cost

### Other
- *(transfers)* more logs around payments...
- do not drop cmds/events
- mutable_key_type clippy fixes
- rebase fixups
- *(networking)* increase timeout for replication fetches
- *(networking)* increase parallel replications
- *(networking)* sort records by closeness
- *(networking)* add some randomness to retry interval for GET
- *(networking)* increase replication fetcher throughput

## [0.9.32](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.31...sn_networking-v0.9.32) - 2023-11-09

### Other
- updated the following local packages: sn_transfers

## [0.9.31](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.30...sn_networking-v0.9.31) - 2023-11-09

### Other
- increase periodic bootstrap interval by reducing stepping

## [0.9.30](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.29...sn_networking-v0.9.30) - 2023-11-09

### Added
- chunk put retry taking repayment into account

## [0.9.29](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.28...sn_networking-v0.9.29) - 2023-11-08

### Other
- *(networking)* use internal libp2p method

## [0.9.28](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.27...sn_networking-v0.9.28) - 2023-11-08

### Added
- *(node)* set custom msg id in order to deduplicate transfer notifs

## [0.9.27](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.26...sn_networking-v0.9.27) - 2023-11-07

### Other
- updated the following local packages: sn_protocol

## [0.9.26](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.25...sn_networking-v0.9.26) - 2023-11-07

### Other
- updated the following local packages: sn_protocol

## [0.9.25](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.24...sn_networking-v0.9.25) - 2023-11-06

### Added
- *(node)* log marker to track the number of peers in the routing table
- *(network)* cache the number of connected peers

### Fixed
- *(network)* use saturating_* functions to track the connected peers

### Other
- *(log)* log the connected peers during peer add

## [0.9.24](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.23...sn_networking-v0.9.24) - 2023-11-06

### Other
- updated the following local packages: sn_protocol

## [0.9.23](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.22...sn_networking-v0.9.23) - 2023-11-06

### Other
- updated the following local packages: sn_protocol

## [0.9.22](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.21...sn_networking-v0.9.22) - 2023-11-06

### Added
- *(deps)* upgrade libp2p to 0.53

## [0.9.21](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.20...sn_networking-v0.9.21) - 2023-11-03

### Added
- *(node)* allow to set a filter for transfer notifications based on targeted pk

## [0.9.20](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.19...sn_networking-v0.9.20) - 2023-11-02

### Other
- *(networking)* use Entry API for query task

## [0.9.19](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.18...sn_networking-v0.9.19) - 2023-11-02

### Other
- updated the following local packages: sn_transfers

## [0.9.18](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.17...sn_networking-v0.9.18) - 2023-11-01

### Other
- *(networking)* remove unused and confusing GetOurCloseGroup SwarmCmd
- *(networking)* update debug for GetCloseGroupLocalPeers
- *(networking)* make NetworkAddress hold bytes rather than vec<u8>
- *(networking)* dont keep recomputing NetworkAddr of record key
- *(networking)* only get KVALUE peers for closeness checks in replication
- *(networking)* only get KVALUE peers when sorting closely
- *(networking)* refactor sort_peers_by_key

## [0.9.17](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.16...sn_networking-v0.9.17) - 2023-11-01

### Fixed
- return with majority

### Other
- log detailed intermediate errors
- throw out SplitRecord in case of FinishedWithNoAdditionalRecord

## [0.9.16](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.15...sn_networking-v0.9.16) - 2023-11-01

### Added
- *(networking)* finish query when stop tracking

## [0.9.15](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.14...sn_networking-v0.9.15) - 2023-11-01

### Other
- updated the following local packages: sn_transfers

## [0.9.14](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.13...sn_networking-v0.9.14) - 2023-10-31

### Other
- *(node)* using unsigned gossipsub msgs

## [0.9.13](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.12...sn_networking-v0.9.13) - 2023-10-30

### Other
- *(networking)* de/serialise directly to Bytes

## [0.9.12](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.11...sn_networking-v0.9.12) - 2023-10-30

### Other
- updated the following local packages: sn_transfers

## [0.9.11](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.10...sn_networking-v0.9.11) - 2023-10-30

### Other
- *(node)* use Bytes for Gossip related data types
- *(node)* make gossipsubpublish take Bytes
- *(networking)* avoid a replication keys clone

## [0.9.10](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.9...sn_networking-v0.9.10) - 2023-10-27

### Other
- updated the following local packages: sn_protocol, sn_transfers

## [0.9.9](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.8...sn_networking-v0.9.9) - 2023-10-27

### Added
- *(networking)* adjust reverification times
- *(sn_networking)* deterministic store cost order

## [0.9.8](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.7...sn_networking-v0.9.8) - 2023-10-26

### Added
- replicate Spend/Register with same key but different content

### Fixed
- throw out SplitRecord error for the later on merge
- client carry out merge when verify register storage

### Other
- expand replication range

## [0.9.7](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.6...sn_networking-v0.9.7) - 2023-10-26

### Fixed
- add libp2p identity with rand dep for tests

### Other
- *(networking)* update libp2p for soon to be deprecated changes

## [0.9.6](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.5...sn_networking-v0.9.6) - 2023-10-26

### Fixed
- typos

## [0.9.5](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.4...sn_networking-v0.9.5) - 2023-10-26

### Other
- pass RecordKey by reference

## [0.9.4](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.3...sn_networking-v0.9.4) - 2023-10-24

### Other
- updated the following local packages: sn_transfers

## [0.9.3](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.2...sn_networking-v0.9.3) - 2023-10-24

### Added
- *(payments)* adding unencrypted CashNotes for network royalties and verifying correct payment

### Other
- nodes to subscribe by default to network royalties payment notifs

## [0.9.2](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.1...sn_networking-v0.9.2) - 2023-10-24

### Fixed
- *(networking)* only validate _our_ transfers at nodes

### Other
- *(networking)* dont retry get_spend validations for UnverifiedData

## [0.9.1](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.9.0...sn_networking-v0.9.1) - 2023-10-24

### Added
- *(networking)* readd a small tolerance to smoothout upload paths

### Other
- *(networking)* kad logging and another content_hash removed
- *(networking)* add SwarmEvent logs
- *(networking)* improve sort
- log and debug SwarmCmd

## [0.9.0](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.42...sn_networking-v0.9.0) - 2023-10-24

### Added
- *(protocol)* [**breaking**] implement `PrettyPrintRecordKey` as a `Cow` type

## [0.8.42](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.41...sn_networking-v0.8.42) - 2023-10-23

### Other
- *(networking)* remove unused content hash

## [0.8.41](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.40...sn_networking-v0.8.41) - 2023-10-23

### Other
- updated the following local packages: sn_protocol

## [0.8.40](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.39...sn_networking-v0.8.40) - 2023-10-22

### Added
- *(protocol)* Nodes can error StoreCosts if they have data.

## [0.8.39](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.38...sn_networking-v0.8.39) - 2023-10-21

### Fixed
- *(network)* return references when sorting peers
- *(network)* prevent cloning of all our peers while sorting them

## [0.8.38](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.37...sn_networking-v0.8.38) - 2023-10-20

### Added
- log network address with KBucketKey

## [0.8.37](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.36...sn_networking-v0.8.37) - 2023-10-20

### Added
- *(node)* allow user to set the metrics server port
- *(client)* stop futher bootstrapping if the client has K_VALUE peers
- *(network)* slow down continuous bootstrapping if no new peers have been discovered

### Other
- *(network)* move bootstrap process to its module

## [0.8.36](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.35...sn_networking-v0.8.36) - 2023-10-19

### Fixed
- *(network)* emit NetworkEvent when we publish a gossipsub msg

## [0.8.35](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.34...sn_networking-v0.8.35) - 2023-10-18

### Other
- logging a node's representitive record_key address

## [0.8.34](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.33...sn_networking-v0.8.34) - 2023-10-18

### Other
- repay for data in node rewards tests

## [0.8.33](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.32...sn_networking-v0.8.33) - 2023-10-18

### Other
- updated the following local packages: sn_transfers

## [0.8.32](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.31...sn_networking-v0.8.32) - 2023-10-17

### Fixed
- *(transfers)* dont overwrite existing payment transactions when we top up

### Other
- remove needless quorum reassignment
- refactor away clunky if statement
- adding comments and cleanup around quorum / payment fixes
- ensure quorum is taken into account for early chunk reads
- *(client)* ensure we only use CLOSE_GROUP closest nodes for pricing

## [0.8.31](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.30...sn_networking-v0.8.31) - 2023-10-16

### Fixed
- consider record split an error, handle it for regs

### Other
- use proper logging funcs

## [0.8.30](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.29...sn_networking-v0.8.30) - 2023-10-16

### Fixed
- *(network)* perfrom bootstrapping continuously to make it well connected

## [0.8.29](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.28...sn_networking-v0.8.29) - 2023-10-13

### Fixed
- *(network)* check `RecordHeader` during chunk early completion

## [0.8.28](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.27...sn_networking-v0.8.28) - 2023-10-12

### Other
- *(client)* dont println for sn_networking

## [0.8.27](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.26...sn_networking-v0.8.27) - 2023-10-12

### Fixed
- *(node)* println->debug statement

## [0.8.26](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.25...sn_networking-v0.8.26) - 2023-10-12

### Added
- *(networking)* return valid result if one found during a timeout

### Other
- remove some low level println
- *(networking)* handle GetRecord kad timeouts

## [0.8.25](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.24...sn_networking-v0.8.25) - 2023-10-11

### Other
- updated the following local packages: sn_transfers

## [0.8.24](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.23...sn_networking-v0.8.24) - 2023-10-11

### Fixed
- handling GetClosestPeers query error branch

## [0.8.23](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.22...sn_networking-v0.8.23) - 2023-10-11

### Added
- showing expected holders to CLI when required
- verify put_record with expected_holders

## [0.8.22](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.21...sn_networking-v0.8.22) - 2023-10-10

### Added
- *(transfer)* special event for transfer notifs over gossipsub

### Other
- feature-gating subscription to gossipsub payments notifications

## [0.8.21](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.20...sn_networking-v0.8.21) - 2023-10-10

### Fixed
- *(sn_networking)* reduce kad query timeout

## [0.8.20](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.19...sn_networking-v0.8.20) - 2023-10-10

### Other
- updated the following local packages: sn_transfers

## [0.8.19](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.18...sn_networking-v0.8.19) - 2023-10-09

### Added
- feat!(sn_networking): remove unroutable peers

### Other
- *(networking)* minor tweaks to reduce mem allocations on Identify
- *(networking)* remove identify clone and collect

## [0.8.18](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.17...sn_networking-v0.8.18) - 2023-10-08

### Fixed
- *(sn_networking)* actually retry PUTs

### Other
- *(sn_networking)* ensure we return on put_record

## [0.8.17](https://github.com/maidsafe/safe_network/compare/sn_networking-v0.8.16...sn_networking-v0.8.17) - 2023-10-06

### Other
- update comments around RecordNotFound
- *(client)* dont println for wallet errors
- *(sn_networking)* do not swallow record retry errors
- *(sn_networking)* retry gets even if we hit RecordNotFound

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
