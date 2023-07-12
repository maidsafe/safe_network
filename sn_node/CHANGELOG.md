# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.83.1](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.0...sn_node-v0.83.1) - 2023-06-07

### Added
- attach payment proof when uploading Chunks

### Fixed
- reduce churn weight to ~1/2mb

### Other
- Revert "chore(release): sn_cli-v0.77.1/sn_client-v0.85.2/sn_networking-v0.1.2/sn_node-v0.83.1"
- *(release)* sn_cli-v0.77.1/sn_client-v0.85.2/sn_networking-v0.1.2/sn_node-v0.83.1
- Revert "chore(release): sn_cli-v0.77.1/sn_client-v0.85.2/sn_networking-v0.1.2/sn_protocol-v0.1.2/sn_node-v0.83.1/sn_record_store-v0.1.2/sn_registers-v0.1.2"
- *(release)* sn_cli-v0.77.1/sn_client-v0.85.2/sn_networking-v0.1.2/sn_protocol-v0.1.2/sn_node-v0.83.1/sn_record_store-v0.1.2/sn_registers-v0.1.2
- *(logs)* enable metrics feature by default
- log msg text updated
- making Chunk payment proof optional for now
- adding unit tests to payment proof utilities
- moving all payment proofs utilities into sn_transfers crate

## [0.83.2](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.1...sn_node-v0.83.2) - 2023-06-08

### Other
- improve documentation for cli arguments

## [0.83.3](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.2...sn_node-v0.83.3) - 2023-06-09

### Other
- provide clarity on command arguments

## [0.83.4](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.3...sn_node-v0.83.4) - 2023-06-09

### Other
- heavier load during the churning test

## [0.83.5](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.4...sn_node-v0.83.5) - 2023-06-09

### Other
- emit git info with vergen

## [0.83.6](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.5...sn_node-v0.83.6) - 2023-06-09

### Fixed
- *(replication)* prevent dropped conns during replication

## [0.83.7](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.6...sn_node-v0.83.7) - 2023-06-09

### Other
- improve documentation for cli commands

## [0.83.8](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.7...sn_node-v0.83.8) - 2023-06-12

### Added
- *(node)* move request handling off thread

## [0.83.9](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.8...sn_node-v0.83.9) - 2023-06-12

### Added
- remove spendbook rw locks, improve logging

## [0.83.10](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.9...sn_node-v0.83.10) - 2023-06-13

### Added
- *(node)* write pid file

## [0.83.11](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.10...sn_node-v0.83.11) - 2023-06-13

### Other
- update dependencies

## [0.83.12](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.11...sn_node-v0.83.12) - 2023-06-14

### Added
- *(client)* expose req/resp timeout to client cli

## [0.83.13](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.12...sn_node-v0.83.13) - 2023-06-14

### Other
- use clap env and parse multiaddr

## [0.83.14](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.13...sn_node-v0.83.14) - 2023-06-14

### Other
- update dependencies

## [0.83.15](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.14...sn_node-v0.83.15) - 2023-06-14

### Added
- include output DBC within payment proof for Chunks storage

## [0.83.16](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.15...sn_node-v0.83.16) - 2023-06-15

### Other
- update dependencies

## [0.83.17](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.16...sn_node-v0.83.17) - 2023-06-15

### Other
- update dependencies

## [0.83.18](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.17...sn_node-v0.83.18) - 2023-06-15

### Other
- update dependencies

## [0.83.19](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.18...sn_node-v0.83.19) - 2023-06-15

### Other
- update dependencies

## [0.83.20](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.19...sn_node-v0.83.20) - 2023-06-15

### Other
- update dependencies

## [0.83.21](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.20...sn_node-v0.83.21) - 2023-06-15

### Added
- add double spend test

### Fixed
- parent spend checks
- parent spend issue

## [0.83.22](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.21...sn_node-v0.83.22) - 2023-06-15

### Other
- update dependencies

## [0.83.23](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.22...sn_node-v0.83.23) - 2023-06-16

### Other
- update dependencies

## [0.83.24](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.23...sn_node-v0.83.24) - 2023-06-16

### Fixed
- *(bin)* negate local-discovery check

## [0.83.25](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.24...sn_node-v0.83.25) - 2023-06-16

### Other
- `--version` argument for `safenode`

## [0.83.26](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.25...sn_node-v0.83.26) - 2023-06-16

### Other
- update dependencies

## [0.83.27](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.26...sn_node-v0.83.27) - 2023-06-16

### Other
- update dependencies

## [0.83.28](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.27...sn_node-v0.83.28) - 2023-06-16

### Other
- update dependencies

## [0.83.29](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.28...sn_node-v0.83.29) - 2023-06-16

### Other
- update dependencies

## [0.83.30](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.29...sn_node-v0.83.30) - 2023-06-19

### Other
- update dependencies

## [0.83.31](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.30...sn_node-v0.83.31) - 2023-06-19

### Other
- update dependencies

## [0.83.32](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.31...sn_node-v0.83.32) - 2023-06-19

### Other
- update dependencies

## [0.83.33](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.32...sn_node-v0.83.33) - 2023-06-19

### Other
- update dependencies

## [0.83.34](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.33...sn_node-v0.83.34) - 2023-06-19

### Other
- update dependencies

## [0.83.35](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.34...sn_node-v0.83.35) - 2023-06-19

### Other
- update dependencies

## [0.83.36](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.35...sn_node-v0.83.36) - 2023-06-20

### Other
- update dependencies

## [0.83.37](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.36...sn_node-v0.83.37) - 2023-06-20

### Other
- update dependencies

## [0.83.38](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.37...sn_node-v0.83.38) - 2023-06-20

### Other
- update dependencies

## [0.83.39](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.38...sn_node-v0.83.39) - 2023-06-20

### Added
- pay 1 nano per Chunk as temporary approach till net-invoices are implemented
- nodes to verify input DBCs of Chunk payment proof were spent

### Other
- specific error types for different payment proof verification scenarios
- creating a storage payment e2e test and run it in CI
- include the Tx instead of output DBCs as part of storage payment proofs

## [0.83.40](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.39...sn_node-v0.83.40) - 2023-06-20

### Added
- *(sn_networking)* Make it possible to pass in a keypair for PeerID

## [0.83.41](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.40...sn_node-v0.83.41) - 2023-06-20

### Added
- *(network)* validate `Record` on GET
- *(network)* validate and store `ReplicatedData`
- *(node)* perform proper validations on PUT
- *(network)* store `Chunk` along with `PaymentProof`
- *(network)* validate and store `Record`
- *(kad)* impl `RecordHeader` to store the record kind

### Fixed
- *(network)* use safe operations when dealing with Vec
- *(node)* store parent tx along with `SignedSpend`
- *(network)* Send `Request` without awaiting for `Response`

### Other
- *(workflow)* fix data replication script
- *(docs)* add more docs and comments

## [0.83.42](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.41...sn_node-v0.83.42) - 2023-06-21

### Added
- provide option for log output in json

## [0.83.43](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.42...sn_node-v0.83.43) - 2023-06-21

### Other
- *(node)* obtain parent_tx from SignedSpend

## [0.83.44](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.43...sn_node-v0.83.44) - 2023-06-21

### Other
- *(network)* remove `NetworkEvent::PutRecord` dead code

## [0.83.45](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.44...sn_node-v0.83.45) - 2023-06-21

### Added
- *(node)* trigger replication when inactivity

## [0.83.46](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.45...sn_node-v0.83.46) - 2023-06-22

### Other
- update dependencies

## [0.83.47](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.46...sn_node-v0.83.47) - 2023-06-22

### Other
- *(client)* initial refactor around uploads

## [0.83.48](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.47...sn_node-v0.83.48) - 2023-06-22

### Added
- *(node)* expose log markers in public api

## [0.83.49](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.48...sn_node-v0.83.49) - 2023-06-23

### Fixed
- trival log correction

## [0.83.50](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.49...sn_node-v0.83.50) - 2023-06-23

### Other
- update dependencies

## [0.83.51](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.50...sn_node-v0.83.51) - 2023-06-23

### Added
- forward chunk when not being the closest
- repliate to peers lost record

## [0.83.52](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.51...sn_node-v0.83.52) - 2023-06-23

### Other
- update dependencies

## [0.83.53](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.52...sn_node-v0.83.53) - 2023-06-24

### Other
- update dependencies

## [0.83.54](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.53...sn_node-v0.83.54) - 2023-06-26

### Other
- having the payment proof validation util to return the item's leaf index

## [0.83.55](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.54...sn_node-v0.83.55) - 2023-06-26

### Added
- *(node)* add handle for Cmd::Response(Ok)

## [0.84.0](https://github.com/maidsafe/safe_network/compare/sn_node-v0.83.55...sn_node-v0.84.0) - 2023-06-26

### Added
- append peer id to node's default root dir

## [0.84.1](https://github.com/maidsafe/safe_network/compare/sn_node-v0.84.0...sn_node-v0.84.1) - 2023-06-26

### Other
- Revert "feat: append peer id to node's default root dir"

## [0.84.2](https://github.com/maidsafe/safe_network/compare/sn_node-v0.84.1...sn_node-v0.84.2) - 2023-06-26

### Fixed
- get_closest_local shall only return CLOSE_GROUP_SIZE peers

## [0.84.3](https://github.com/maidsafe/safe_network/compare/sn_node-v0.84.2...sn_node-v0.84.3) - 2023-06-27

### Other
- update dependencies

## [0.84.4](https://github.com/maidsafe/safe_network/compare/sn_node-v0.84.3...sn_node-v0.84.4) - 2023-06-27

### Other
- update dependencies

## [0.85.0](https://github.com/maidsafe/safe_network/compare/sn_node-v0.84.4...sn_node-v0.85.0) - 2023-06-27

### Added
- append peer id to node's default root dir

## [0.85.1](https://github.com/maidsafe/safe_network/compare/sn_node-v0.85.0...sn_node-v0.85.1) - 2023-06-28

### Added
- *(node)* dial without PeerId

## [0.85.2](https://github.com/maidsafe/safe_network/compare/sn_node-v0.85.1...sn_node-v0.85.2) - 2023-06-28

### Other
- update dependencies

## [0.85.3](https://github.com/maidsafe/safe_network/compare/sn_node-v0.85.2...sn_node-v0.85.3) - 2023-06-28

### Added
- make the example work, fix sync when reg doesnt exist
- rework permissions, implement register cmd handlers
- register refactor, kad reg without cmds

### Fixed
- rename UserRights to UserPermissions

## [0.85.4](https://github.com/maidsafe/safe_network/compare/sn_node-v0.85.3...sn_node-v0.85.4) - 2023-06-28

### Added
- *(node)* increase node event channel size

## [0.85.5](https://github.com/maidsafe/safe_network/compare/sn_node-v0.85.4...sn_node-v0.85.5) - 2023-06-29

### Other
- update dependencies

## [0.85.6](https://github.com/maidsafe/safe_network/compare/sn_node-v0.85.5...sn_node-v0.85.6) - 2023-06-29

### Added
- *(node)* write secret key to disk and re-use

## [0.85.7](https://github.com/maidsafe/safe_network/compare/sn_node-v0.85.6...sn_node-v0.85.7) - 2023-07-03

### Added
- append SAFE_PEERS to initial_peers after restart

### Fixed
- *(CI)* setup stable SAFE_PEERS for testnet nodes
- *(text)* data_churn_test creates clients parsing SAFE_PEERS env

### Other
- various tidy up
- reduce SAMPLE_SIZE for the data_with_churn test
- tidy up try_trigger_replication function

## [0.85.8](https://github.com/maidsafe/safe_network/compare/sn_node-v0.85.7...sn_node-v0.85.8) - 2023-07-04

### Other
- demystify permissions

## [0.85.9](https://github.com/maidsafe/safe_network/compare/sn_node-v0.85.8...sn_node-v0.85.9) - 2023-07-05

### Added
- carry out validation for record_store::put

## [0.85.10](https://github.com/maidsafe/safe_network/compare/sn_node-v0.85.9...sn_node-v0.85.10) - 2023-07-05

### Fixed
- *(node)* verify incoming `Record::key`

## [0.86.0](https://github.com/maidsafe/safe_network/compare/sn_node-v0.85.10...sn_node-v0.86.0) - 2023-07-06

### Added
- add restart func for node process
- remove option from `--log-output-dest` arg
- introduce `--log-format` arguments
- provide `--log-output-dest` arg for `safenode`

### Fixed
- use SAFE_PEERS as fall back initial peers for non-local-discovery

### Other
- tidy remove_file call
- clear out chunks and registers
- use data-dir rather than root-dir
- incorporate various feedback items

## [0.86.1](https://github.com/maidsafe/safe_network/compare/sn_node-v0.86.0...sn_node-v0.86.1) - 2023-07-06

### Added
- client upload chunk using kad::put_record

## [0.86.2](https://github.com/maidsafe/safe_network/compare/sn_node-v0.86.1...sn_node-v0.86.2) - 2023-07-06

### Other
- update dependencies

## [0.86.3](https://github.com/maidsafe/safe_network/compare/sn_node-v0.86.2...sn_node-v0.86.3) - 2023-07-07

### Other
- update dependencies

## [0.86.4](https://github.com/maidsafe/safe_network/compare/sn_node-v0.86.3...sn_node-v0.86.4) - 2023-07-07

### Other
- update dependencies

## [0.86.5](https://github.com/maidsafe/safe_network/compare/sn_node-v0.86.4...sn_node-v0.86.5) - 2023-07-07

### Other
- update dependencies

## [0.86.6](https://github.com/maidsafe/safe_network/compare/sn_node-v0.86.5...sn_node-v0.86.6) - 2023-07-07

### Other
- adapting paid chunk upload integration tests to new no-responses type of protocol
- adding integration tests for uploading paid chunks and run them in CI

## [0.86.7](https://github.com/maidsafe/safe_network/compare/sn_node-v0.86.6...sn_node-v0.86.7) - 2023-07-10

### Other
- update dependencies

## [0.86.8](https://github.com/maidsafe/safe_network/compare/sn_node-v0.86.7...sn_node-v0.86.8) - 2023-07-10

### Added
- read peers from SAFE_PEERS if local discovery is not enabled
- faucet server and cli DBC read

### Fixed
- use Deposit --stdin instead of Read in cli

## [0.86.9](https://github.com/maidsafe/safe_network/compare/sn_node-v0.86.8...sn_node-v0.86.9) - 2023-07-10

### Added
- client query register via get_record
- client upload Register via put_record

## [0.86.10](https://github.com/maidsafe/safe_network/compare/sn_node-v0.86.9...sn_node-v0.86.10) - 2023-07-10

### Added
- *(node)* remove any data we have from replication queue

### Other
- *(node)* cleanup unused SwarmCmd for GetAllRecordAddrs
- add more logging around replication

## [0.86.11](https://github.com/maidsafe/safe_network/compare/sn_node-v0.86.10...sn_node-v0.86.11) - 2023-07-11

### Other
- update dependencies

## [0.86.12](https://github.com/maidsafe/safe_network/compare/sn_node-v0.86.11...sn_node-v0.86.12) - 2023-07-11

### Other
- *(node)* only log LostRecord when peersfound

## [0.86.13](https://github.com/maidsafe/safe_network/compare/sn_node-v0.86.12...sn_node-v0.86.13) - 2023-07-11

### Other
- update dependencies

## [0.86.14](https://github.com/maidsafe/safe_network/compare/sn_node-v0.86.13...sn_node-v0.86.14) - 2023-07-11

### Fixed
- prevent multiple concurrent get_closest calls when joining

## [0.86.15](https://github.com/maidsafe/safe_network/compare/sn_node-v0.86.14...sn_node-v0.86.15) - 2023-07-12

### Other
- update dependencies

## v0.1.0 (2023-05-04)

### Chore

 - <csr-id-3cff43ca4fce96055a4f506425f7b1af76057188/> code cleanup
 - <csr-id-ecebc7a93b014aac397eca3d5149e5583e8be04f/> fixing doc dep
 - <csr-id-f1d8be8cd15506df0432da9179f1de0b1c0b8f67/> fix doc typos
 - <csr-id-6a53c1e7b54c7e93e485510556c7d8dd6a0eec3b/> improve logging for parent validation
 - <csr-id-0487be41aeeb96d0945e2b76a0045e3b19ffcf17/> improve spend storage logs
 - <csr-id-fbee86db94bf77cbf27a28f803f04005c5ac51cd/> fix incorrect log msgs
 - <csr-id-e727feca3eb4626d05d4989e38366a4376dde127/> improve msg docs
 - <csr-id-030fc25c8d9fa5e54f6937844cd6a633aff173cd/> disable logging in spend e2e test
   - It is too verbose and hinders reading the flow.
 - <csr-id-31b7f668a80f026ee768c9738282cc81dcb3f00b/> remove wait before verifying tranfer
 - <csr-id-70742c272fa8a92cdb3b15a14b803ee993e14aa9/> traces to println for wallet file
 - <csr-id-4de0b10e4f5a063427e4296c96e90e2f966bd621/> traces to println for wallet file
 - <csr-id-3860a813ad543d7c7c436205453d90c484f1d4f1/> traces to println for wallet localstore
 - <csr-id-ad182d8d9e103e662a70d67c316a3a8fbe2b42f2/> change traces to println for register cli
 - <csr-id-dae3ac55b164fc5ea73458a53728564bee6d03b2/> change traces to println for files cli
 - <csr-id-3fe59434384448a5d9c5b934710db45aabb3e22a/> change traces to println for wallet cli
 - <csr-id-ff72c32023b46df0a0f320f6b5480939da9b40b2/> typo fix
 - <csr-id-36a92f7dc4c9b4e97a1f45b755cde764af536305/> clarify current state of tx queue docs
 - <csr-id-35846da7e59d2f2c6cdef8538b813d19cac21680/> remove unnecessary double store of hash
 - <csr-id-f61119e255828a6222398db470e74aee8ad88d3e/> replace println with trace in local wallet
 - <csr-id-d0d536d6e66e43766fdb009cfe8672f738a986a9/> use the error in local wallet send
 - <csr-id-ddd438c66e2b5fa71ea2b0d1e57d732af4deb447/> increase comment line width
 - <csr-id-23a309d1d3e2c1c6b928cfd7c2ebda9423798e77/> document pending txs
   - This explains its workings, limitations and todos.
 - <csr-id-6ab6f6ab05a18ba4b00c5799c1ecf8a880426cb6/> update wallet docs
 - <csr-id-00248dd8264ac75f6967be19ebd9f34ad7ebfdcd/> minor style fixes
 - <csr-id-6979d05e5574163b47d6184d217c993a1c72ee3d/> disable some very verbose log outputs
 - <csr-id-5b28b75e8f65ff2f4ea33fec7c63e813a64c3c4d/> simplify faucet cli dir structure
 - <csr-id-360cd85cd0c3ce2acad5438a22cea1a2650de3f8/> remove limit-client-upload-size
 - <csr-id-396b3e9f06a8d76af521552a5ffe1eb7eb57078b/> rename RequestResponse to MsgReceived
   - This follows the event naming convention and is directly communicating
   what happened.
 - <csr-id-63806e3d95bdcfbf97e00bb57eb93ff6c8c092fb/> skip get spend debug due to very verbose
 - <csr-id-7880e416140c10600ff9f35fb4b1ad195de336c8/> improve fn naming
 - <csr-id-79174f45852add379610480301dd8ad888dbb164/> remove params suffix from struct name
 - <csr-id-d049172fff516df5d22b4a32e74cfe828704ac4d/> move logic in block to fn and clarify docs
   - This further helps with readability.
 - <csr-id-d7807febbadf891e320c5a265743e14d698086d5/> move client transfer traits to own files
   - This helps with code readability.
 - <csr-id-a2f054a3d0deb560cfea2208fcea0d1af8cc55f8/> remove unnecessary allow that snuck in
 - <csr-id-b51a8b04d3a99af93714da9f68f12c360176ce1c/> fix copy paste doc error
 - <csr-id-d774fb80860f2747e583fc511a8d84e6a5cde237/> move temp allow large err to one place
 - <csr-id-f81b5a34a0c166a0dbd91618205a1a61bc1aa87a/> use print instead of log in client api
 - <csr-id-f8c29751ffcaecb3401715dd0f5a6d87f5e70146/> impl display for data address
 - <csr-id-8a43ddfe28408e032b481fb8d88c1234df17be5e/> impl display for cmd
 - <csr-id-e0ee848017cd41a66bad18e1004644e982f7e41e/> impl display for query
 - <csr-id-2b9bb9fc0052cb68801973aa342ab8ec6bfc2241/> impl display for spendquery
 - <csr-id-71514749883e62c90d0ecfacf371499c8373d054/> log individual spend errors at client
 - <csr-id-688fe6bbea6db783bae6c601cb6fbf05cc57d16c/> improve error msg too few ok responses
 - <csr-id-13180738c4ca1440a91cba7554208e1e0735c5ec/> impl display for response
 - <csr-id-510b4cc1d19c678f4c8ae984b5c5835662c69cda/> impl display for queryresponse
 - <csr-id-3bc906b02dfeb18149d76c8e0d5f833c5a74a212/> rename faucet cmd variant to ClaimGenesis
   - This is more accurate, as the cmd takes all genesis amount and
   transfers it to the faucet wallet.
 - <csr-id-114a54c8def8f131a22b810b9507f06a4bc3a13e/> remove unnecessary file
 - <csr-id-a63b2599bd49f6bcece4d55345a98379e11d59b6/> rename dbc create fns
 - <csr-id-5f6aace3c14160b616fe705f2998cc161300bffb/> add setup for transfer e2e tests
 - <csr-id-dbe2165f05dce1c65b42835eb3763e725cf086a1/> rewording / renaming refactors for chunk provider setup
 - <csr-id-88223b77527c5645228a4a00cba4cd51e184fe06/> update MemoryStore cfg for larger record sizing
 - <csr-id-6fb46aae8acefbfa130d152aaabf6c429c9bf630/> fix doc typo
 - <csr-id-ec859ec379edc47718929a7e188590e0686b03b1/> fix required fee ctor doc
 - <csr-id-fe86af5632cce2639d36ce5b735efc8d70e301b9/> cleanup transfer errors
 - <csr-id-076cf5509a1afedbc416c37a67632abe972c168c/> update faucet mod docs with example
 - <csr-id-044551d5aa295d9e2bc3d2527ca969a32858cc2d/> update incorrect cli cmd docs
 - <csr-id-fe4fa10c26f7e284a4806f19dfb915b6d105dceb/> clarify test fn doc
 - <csr-id-16e389da94aac51c46cc13c23ece1f54fa152ff9/> add faucet module docs
 - <csr-id-08c65ffc2b6d90ef843b21e157927bbb23406ec9/> remove unused files
 - <csr-id-3ee3319d18dcd29b8d16c4ae24fbfad1be0e1e1c/> rename kadclient to safe
 - <csr-id-f6e1c532171e72f52026195431cc0e836627f513/> move kadclient and its cli to own dir
 - <csr-id-72c67ba9199b3f105bd398cf34e0be88afedc5db/> improve match clause
   - Uses a neater code design for the task.
 - <csr-id-3db9b55223bcfa6e81df0ec23d36b3b2f7d68d44/> clarify test name
 - <csr-id-504f4ee5b10b75138044b1af8150825b53f776d3/> make cli wallet cmd a bit less technical
   - Also clarifies the necessary steps to be taken by the user.
 - <csr-id-0559ca06fb3d00e80e76d9736b030a543e34fc4c/> clean up and add assertion to test
   - Some cleanup of created_dbc_to_file_can_be_deposited test.
 - <csr-id-04a724afbc9495937b8be7ab905f9695e68ad398/> create received_dbcs dir by default
   - This allows user to add dbcs first thing they do after wallet was
   created.
 - <csr-id-fc895e3577a94f620bf398b6cb3b2f189f34ebd0/> update cli to not take path
 - <csr-id-9e11748a191b4432499ceb6beded2a9dda15cf56/> move get client dir to kadclient
   - Updates wallet_cmds to take root dir as arg.
 - <csr-id-e9ce090c2361dcd49400112f8d2e3d29386602d7/> move missed domain logic
   - This should have been moved from protocol earlier but was missed.
 - <csr-id-fb095b5e63f826f4079ba2c7797a241969346d0b/> additional review comment fixes
 - <csr-id-dfe80b902f0e8f6803eb836aeb9c81363ae183a9/> apply fixes from review comments
 - <csr-id-bc7bbb3a502f2e5d2c673678e2f7bc132bc4b490/> add missing asserts to reg tests
 - <csr-id-1b474d5d5ca952dba9a785b31df6201a62c1b34e/> remove unused dep
 - <csr-id-69c13458a737221d75fccc73d8e534331d4dbe2e/> minor comment fixes
 - <csr-id-4fbddd23e174329dc97f8d66c387b5544366e620/> add missing comments and remove old
 - <csr-id-435208c7dc1c51e1d51f730c84ac648cff1026a1/> remove unnecessary error mappings
 - <csr-id-66ba179061f5dcd13369edd7a569df9c0e1e5002/> remove unused macro
 - <csr-id-e39363c8418e9e738c8e5380208666c20cbfed5d/> remove unused log line
 - <csr-id-05f5244afdd588ff71abcf414f3b81eb16803883/> fix doc refs
 - <csr-id-b96904a5278ab1105fa4de69114151b61d0ada70/> remove file logs from client cli
   - Instead do something like enable -vvvv and pipe it into a log file.
 - <csr-id-78061111dc92f86ba976b8e75f49f02d3276d6d7/> additional error variant cleanup
 - <csr-id-422345591d989c846151ccca36d0af8b67aaeccf/> move chunk into chunks in storage
 - <csr-id-b198a36220c6a5fe39227c72b5a050dcb351c0cd/> move register into registers in storage
 - <csr-id-267399c6aa597c114706532fddcaf5167dd69441/> move register into storage mod
 - <csr-id-7201b6186a520bc3ca23e07cfc287e8a7197a5af/> move address into storage
 - <csr-id-01f75ac286736ec8df346aa41328604dbb68af38/> remove unnecessary indirection for regstore
 - <csr-id-651c7f53928847cf604bc1b1a9f3eb2df2f081ae/> move storage to protocol
 - <csr-id-5e943fe0c828a56a0f6ba047dbf378af605d43ac/> don't double handle cfg variant
 - <csr-id-bb66afeaa2151427d39d794bbdb9916c9e116c24/> add fixes from review comments
 - <csr-id-0b810c3706c04417e10ec1fd98e12a67b1b686c9/> update readme client cli user instructions
 - <csr-id-23b4a0485a744f524666095cb61e8aef63a48fdd/> fix cli files upload and download
 - <csr-id-291a38a492ea33c757a12e43b0a10963d9967cd4/> remove unused dep
 - <csr-id-d5375254ebd47e223f98bcb90df9b155f914374b/> simplify amount parsing for wallet send
 - <csr-id-5f22ab864ac0c7de045c27d75a712e13f5a4723b/> move subcmd impls to their definition
 - <csr-id-452c0df869b3398673bb61a0c9f19509f39ad044/> move wallet ops to kadclient
   - We didn't want a separate binary for this.
 - <csr-id-3b1ab1b7e8e0ce37bee64b462d5f230bf079f65b/> move respective ops into fns for wallet
   - This makes the main fn less cluttered.
 - <csr-id-35a01e7fd9942964f01746be54587e65444b95d8/> move respective ops into fns
   - This makes the main fn less cluttered.
 - <csr-id-18f2e869f85fb096d3998e89ea29e54c7c7902d4/> improve naming
 - <csr-id-1457a453341e35ad3fbf426b4e1fa4a57a753761/> ensure testnet launch fails if build fails
 - <csr-id-ab5c82e2fe63b43f4c8c35848cae8edc0dd2b6d2/> fix typo
 - <csr-id-ffe9dfe50b7fcec30b5fe6103d033b042b1cb93f/> doc updates
 - <csr-id-66bf69a627de5c54f30cb2591f22932b2edc2031/> rearrange the code
 - <csr-id-ee46ba19ab692dbdbab5240c1abea4be24a2093a/> use load_from in tests
   - This auto-generates a new mainkey.
 - <csr-id-33b6a872a3f15087e78ec9df8b3aa708960a173b/> clarify the need for NotADoubleSpendAttempt
 - <csr-id-9a1a6b6d460cd4686044f4ccd65f208c5013e1ff/> misc fixes from pr 95 comments
 - <csr-id-714347f7ceae28a3c1bfcbcf17a96193d28092ae/> make long error variants simpler
 - <csr-id-7876c9d02f4cccf2f3d0f9c23475100927a40ece/> clarify docs
 - <csr-id-3c8b58386dd90499ee65097378d5edccab801f3d/> remove unnecessary indirection
   - The nesting doesn't serve any purpose, and is not very accurately
   named.
   - The contents are all directly parts of the protocol.
 - <csr-id-dd845b970c2e475b0aec8081eba28ce6f1bc6015/> distinguish transfer modules
 - <csr-id-bf72aff8e265cb67d0a48e4f5979370e7b77ba15/> rename Dbc cmd to SpendDbc
 - <csr-id-8039166f53839cb56d421421b45b618220f19fd1/> update and extend docs
   - Also an attempt at better naming for wallet variable of created dbcs.
   Still not entirely satisfactory though..
 - <csr-id-c800a2758330b91559980d11ad05d48936c5a546/> use latest sn_dbc
 - <csr-id-b075101a173211e422544db9f11597a1cd770eab/> additional cleanup and organisation
 - <csr-id-82323fbdb1810bcf1e4c70ed54550499231434bf/> improve file org and some cleanup
 - <csr-id-b19cafca11cf4469e3f235105a3e53bc07f33204/> update due to libp2p new version
 - <csr-id-55e385db4d87040b452ac60ef3137ea7ab7e8960/> fix old terminology in comment
 - <csr-id-3a6c5085048ae1cc1fc79afbfd417a5fea4859b6/> remove commented out tests
   - We can add these properly later.
 - <csr-id-2c8137ce1445f734b9a2e2ef14bbe8b10c83ee9a/> comment updates
 - <csr-id-ef4bd4d53787e53800e7feef1e0575c58c20e5e1/> move double spend same hash check
 - <csr-id-139c7f37234da8b79429307b6da6eedbac9daae6/> remove some paths to simplify the code
 - <csr-id-351ce80063367db32778d1384896639cd34b4550/> remove unnecessary conversion of hash
 - <csr-id-a1702bca4e4b66249f100b36319dc7f50a1af8fc/> reference latest version of sn_dbc
 - <csr-id-08db243d8db1e5891cc97c2403324cc77e3d049c/> remove empty file
 - <csr-id-2161cf223c9cdfe055b11bf2a436b36077392782/> update to released sn_dbc
 - <csr-id-7fd46f8f391be0ef315d0876f3d569c806aa3b70/> various minor adjustments
   While making an effort to understand the node start up and the different async tasks in use, I
   noticed small ajustments I thought I could make to perhaps improve clarity.
   
   * Rename: `NetworkSwarmLoop` to `SwarmDriver`, which then provides the loop in its `run` function.
   * Use GPT-4 to document `SwarmDriver` and its public functions. Did not need any adjustment.
   * Rename some variables in `SwarmDriver::new` for extra clarity.
   * Rename `Node::node_events_channel` to `Node::events_channel` since it's part of the `Node` struct.
   * Use GPT-4 to document `Node` and its public functions. Did not need any adjustment.
   * Removed comments that appeared to provide limited value.
 - <csr-id-9b52e333699454179f298a44d2efd1c62bf49123/> fix naming
 - <csr-id-5cd9f4af674a1e19ea64b1092959477afdeb4040/> use tokio everywhere
 - <csr-id-29f726ad86c111f3ac7f4fa858fe7f5ba6b2996d/> disable random restart
 - <csr-id-ac754fdf24919065cc1292f4df7e6dab31388fcd/> remove chunk specific api
   simplifies to one  api that takes
   ReplicatedData
 - <csr-id-9bbee062afe133dea986350ae8480b63bdce131f/> flatten errors
   Moves storage errors up into the protocol to avoid
   duplication there. Makes explicit when we're
   simply serialising an error from bincode/hex etc
 - <csr-id-de04d62f6dc155616c14e0f4a07f3b8205398b1b/> remove deps, remove EnvFilter
   We can specify log levels in the code as needed without having to bring in
   EnvFilter (and regex).
   
   Although right now regex is used elsewhere, we can hopefully remove that large dep
 - <csr-id-0e9bc3da11878ac9357eb76c8cf61fd2a83a8735/> use tokio executor all over
   right now we mix w/ async-std
 - <csr-id-d748fcd6e6c3ba604fb898b3be8b73e96270e993/> fix naming
 - <csr-id-ba7c74175e7082f6a2d4afc64a85be2c56b9d8c9/> add docs + clippy fixes
 - <csr-id-3374b3b6bcd2e010ef31ec46c5bb87515d8ba6f7/> include reference impl
 - <csr-id-f063f8442608f074dbaf5c4b15dcb419db145fcf/> kadnode attempt w/ tcp
 - <csr-id-b827c2028f59191a7f84a58f23c9d5dfb3bd7b11/> make response stream optional again, respond to sender over stream if existing
 - <csr-id-0bcce425ef56b54095103c5a8cfb3787b8a94696/> refactor out stable set update from msg processing
 - <csr-id-af56c5ec20c84516e2330b9d4077dc30c696df4e/> refactor out stable set msg received event extraction
 - <csr-id-9bbadd672ebb1aa4bb66a538b921f5c3691fe12a/> update gitignore to remove trunk
 - <csr-id-f772949320519c868a5e2ffc3b611aa138567afd/> cargo fix
 - <csr-id-e40ac52e83be846c2c026d9618431e0269a8116b/> convert safenode to bin
   This should get us ready to incorporate testnet bin for launching nodes
 - <csr-id-0074ea6ce8f4689c9a6bc42e94539fd42e564a7a/> create a basic workspace for the repo
 - <csr-id-6a318fa7af40360c2ea8b83f670ce3f51b0904bc/> convert safenode to bin
   This should get us ready to incorporate testnet bin for launching nodes
 - <csr-id-368f3bcdd1864c41c63904233b260b8d2df0a15a/> create a basic workspace for the repo

### New Features

 - <csr-id-85b359b686facefd65c56be1d54ca5ef0a9f10f6/> write client logs to tmp dir by default.
   Also removes a swrm cmd log which would log full record values.
   (This is a libp2p behaviour, so best just not to log atm)
 - <csr-id-d7e344df6aaca3bef75d7c9d90edca7d39771194/> add passed in peer to routing table
   Also fixes a problem where the client thinks it's connected to the
   network, while we're not yet adding any node that we discover via mDNS.
   We need to wait for the Identify behavior to kick in.
 - <csr-id-5693525b2cb4c285fd80137ce1528ab6f2a69358/> add identify behaviour
   Using the identify behaviour to exchange node information and adding addresses to the routing table based on that.
 - <csr-id-fdeb5086a70581abc4beb05914dd87b8ed791ffb/> add AlreadyDialingPeer as error
   Without returning this error, the receiver will get an error because we
   drop the sender without sending anything on the oneshot channel.
 - <csr-id-5bd7fb9f486fc85af8dfbc155e6435415b152c10/> moving all transfer fees related types onto the protocol crate/mode
 - <csr-id-bab05b011c8e5ecf70d2a6c61d9289eebc78f533/> isolating all 'protocol' types from their implementations
   - All types/structs that strictly belong to the SAFE protocol are being kept
   in the 'protocol' mod/crate except for those coming from 'rut-crdt' crate
   which will be done in a follow up PR.
 - <csr-id-e3b55c9cbbdf26e3018b736d29a31887c5355811/> allow clients to dial specific network peers
 - <csr-id-2507695a7af51de32d40ab90981975e0372916c3/> fire and forget broadcast of valid spend
 - <csr-id-ee0d5feedbfe80c513520ff6a9d914815b8087ee/> impl fire and forget request in network
 - <csr-id-772b97208b7c756b1ecc25377e80d9d53baceff4/> impl spend store exists
   - This allows for skipping unnecessary paths, such as adding the spend
   to the priority queue again.
 - <csr-id-035c21b93ec8f03a2fa9d581a57d4a4a9bc9c707/> node to broadcast storage events for Chunks, Registers and Spends
 - <csr-id-c2ef1f6d5defc075f80dfc0d0f6d6aec9d511d32/> broadcast spend as node confirm its validity
 - <csr-id-d95bf6cdbd10f907112bf2f707bbf0d2f7f8f235/> resend pending txs when other transfer made
   - This shall ensure that we always get our pending txs out before
   doing new ones.
   - Note though, as documented, that there are still cases where we
   actually cannot get a pending tx to be stored as valid. It needs
   a way to later validate and clear out the list, if such a state is
   reached.
 - <csr-id-3fc3332e74323f4c635a89527075d9b6c61abcc5/> add a failing spend to the pending txs list
 - <csr-id-17849dcbbc8bea681a3d78a62ba7613877eab81a/> set timeout through the `RequestResponse` behaviour
 - <csr-id-903c59f09f8520dad129fcf97685877b0bfe78f7/> fast refresh delays
 - <csr-id-ad7de377a0aa0e47c09778ed1f2951a77e5eed90/> add client cli cmd balance
 - <csr-id-e5bf209b5c1bcea0a114f32a1737bb0b4101d5c7/> add client cli cmd address
 - <csr-id-1513ef5f33993cc417e969d36ca50055884f10ea/> impl early return for majority get spend
 - <csr-id-a71b9ffca53a7e5a7e1a75f38c00c4a59c8acbae/> impl early return for majority spend ok
 - <csr-id-cab992f23070894107696a20de12d94e7a381dea/> identify genesis spend
   - This allows for the base case of the genesis to pass validation
   (which it would otherwise fail, as its src tx doesn't exist).
 - <csr-id-8270cdb96888bdf35f896ec0ce4ff9a27a6d6274/> load genesis dbc from hex
 - <csr-id-8bf5d578bec4d72dac1c412c2b2d456cd9f4e212/> differentiate missing fee errors
 - <csr-id-abfd1a621bb00382549b1d4b93a815dfb9a2debf/> use deterministic blinding for genesis
 - <csr-id-959081620e1787accb4959bee6b01dfff7fe6024/> verify a dbc is valid in the network
 - <csr-id-b1d5f5c5c0cbe07e0ec1c4ed801c617d059c5ed6/> verify close group majority ok a spend
 - <csr-id-b0d9d4521bc1c05b21fc659a593be7369a94574d/> impl verification of deposited dbc
 - <csr-id-fee76e8650647b32dc4bd4ee95e2205398f4e04e/> remove chunk storage
   We use MemoryStore and providership from the kad impl now
 - <csr-id-55cef547a71b524e1bd1a17b98105bd6867de769/> use provider and MemoryStorte for retreiving chunks
 - <csr-id-ddb8ea170c5ead4988e9aecd8d21768f5dfe34b4/> use kad MemoryStore for registering Providership of chunks
 - <csr-id-4eeeddc415cd625a898b7af8b6b19b7a6b91dfd2/> initial setup of KademliaConfig for provider usage
 - <csr-id-16e60498965deb0b209429a50ca54016095f2879/> example cmd querying network info from node's RPC service
 - <csr-id-66eeff38da7cdcfd8b3e2230ca1e654d15cfd1e5/> exposing an RPC service to retrieve node connection info
 - <csr-id-e9bfec3fcd300a714733a7718206797e5116d80d/> impl fee cipher decrypt for wallet
   - TODO: Adding FeeCiphers to the wallet API is not good. Refactor to
   remove it ASAP.
 - <csr-id-caac9e99d0bc763ee3b6c3861ba4151bdcf947a7/> impl new_with_genesis for Transfers
 - <csr-id-bb376bcc1320d8477daab3ce3b76b08c090114e6/> impl new_with_genesis for SpendStorage
 - <csr-id-a17876e9190b4db6d4859736f569770827d0b2b1/> impl wallet sign
 - <csr-id-044b05d34c5686076f9673c2cabbd76cd6902a37/> add testnet faucet to cli
 - <csr-id-705c67f672f4be870c4aae6b82c33f7cb7d0a89f/> store created dbcs as hex to file
   - This allows a client to send these files to the dbc recipients, for
   them to deposit the dbcs to their wallets.
 - <csr-id-71acb3cc8383e4b8669c0c95cb302d05b1f8c904/> allow downloading files to file system
   - Improves the cli ergonomics.
   - Unique txt doc for each set of files uploaded.
   - Always downloads files to the client path.
   - Updates ci tests.
 - <csr-id-6916b4e1af97c982a77a649be7889fcd0b4637e8/> spends drive storage
   - This stores spends persistently on drive.
 - <csr-id-30586c9faa43489e7565164c768fa9afb3959e88/> register drive storage
   - This stores registers persistently on drive.
 - <csr-id-1a8622cb26db066481a9d12fce1065a1d57abcb4/> chunk drive storage
   - This stores chunks persistently on drive.
 - <csr-id-69d1943d86870d08a9e1067a05b689af7e32711b/> detect dead peer
 - <csr-id-74d6502ebbf76cf3698c253e417db562c6a11e3b/> fix subcmds
 - <csr-id-420ee5ef7038ea311bfe6d09fd6adf0c124a1141/> adding example client app for node gRPC service
 - <csr-id-5b266b8bbd1f46d8b87917d0573377ff1ecaf2f7/> exposing a gRPC interface on safenode bin/app
 - <csr-id-0b365b51bba9cde4a9c50f6884f5081d239eed6d/> impl simple cli for wallet ops, sending
   - Adds send operation.
   - NB: Does not yet store the created dbcs, for giving them to the
   recipients out of band.
 - <csr-id-cf4e1c2fbf6735641faa86ec6078b2fe686adba7/> impl simple cli for wallet ops
   - Adds deposit operation.
 - <csr-id-6a4556565df6689a0bfe0450fc9ac69d74b23ec0/> dial peers on startup
   We dial optional peers on startup that will get added to our routing
   table et al. This will cause our node to get booted by specifying a
   bootstrap node address.
 - <csr-id-4c4b19e55892ece1bd408a736bd21ea5c6ea3bf1/> log when a peer disconnects
 - <csr-id-edff23ed528515ea99361df89ea0f46e99a856e8/> register spends in the network
   - This is the final step in making a transfer valid in the network.
   - Fees are paid to nodes.
   - NB1: Some additional validation of responses is needed to make sure we
   error if not enough nodes could handle the request.
   - NB2: Nodes still need to store the rewards in their wallet, TBD.
   - NB3: There are still some code reuse work to be done between
   transfer online and offline files.
 - <csr-id-4e9c0076f010bf796fbef2891839872bfd382b49/> add online transfer logic
   - This includes fees.
 - <csr-id-e57920279f352d8c02139138e4edc45556228ad4/> instantiate wallet in client
 - <csr-id-33b533f99af1b1e20cea5868636b478df9aed9ec/> store and load from disk
   - As a temporary solution, the serialized wallet can be stored to disk.
   - Next the wallet ops will be stored to disk as a Register.
 - <csr-id-16ea0a77993015cf9f00c4933edca0854e13cc87/> extend kadclient to up-/download files
   - It can now upload and download entire files, instead of small chunks.
   - Additionally, the files apis are encapsulated in their own struct, as
   to not bloat the client api.
 - <csr-id-72554f3f3073189d9c59afb23f98e6cc8c73c811/> additional Register client API
 - <csr-id-75ee18f11787d31b0126dcec96142e663f21da8d/> connect spends, fees and the msgs
 - <csr-id-e28caece21bf214f3ad5cead91cbfe99476bb8b9/> add the transfer fees and spend queue logic
 - <csr-id-197e056ed1628be48c6d4e115fbeb1f02d167746/> impl reissue for tests
   - Implements reissuing without fees and without contact with network.
 - <csr-id-ae0c077f7af8c63cef28a92ad41478a7bb5fef68/> implement local wallet
 - <csr-id-fd7b176516254630eff28f12a1693fc52a9a74a8/> Register client API
   - Supports offline-first type of operations for users to work
   on Registers offline and syncing local changes with remote
   replicas on the network when they decide to.
   - Public APIs for Register in offline mode are all sync, whilst those that
   work 'online', i.e. syncing right after each operation is made, are all `async`.
   - Allow users to resolve/merge branches of Registers entries if
   their use case requires a single branch of values as content.
 - <csr-id-4539a12004a0321b143d5958bf77b1071e91708d/> specify ip and port to listen on
 - <csr-id-a6b9448a113bdbdaa012ffa44689f10939ddfe37/> random query on peer added
 - <csr-id-33082c1af4ea92e507db0ab6c1d2ec42d5e8470b/> add file apis and self encryption
   - This adds all file apis for chunking and storing files,
   as well as retreiving and unpacking chunks.
 - <csr-id-fc9524992474abee593c1be203e640cbcb0c9be9/> validate parents and doublespends
   - Adds extensive checks on spends and their parents.
   - Also makes sure that detection is broadcasted to relevant peers.
   - Extends the Request enum with an Event type, used to broadcast facts /
    things that happened.
 - <csr-id-179072ec7c66fe6689b77d47ef6bf211254054b6/> count self in the close group
 - <csr-id-6ef0ef9c7375bb6d690bd464269a1f6c38e188af/> implement Client API to use a Kad swarm in client-only mode
 - <csr-id-6cc84506304c895cda63d7588d9b938aa8aa6039/> use close group var
   - This allows verification that we got enough nodes, according to our
   protocol.
 - <csr-id-2e781600e52321092ce5a903a9f9106e5374d17d/> boundary of get_closest_peers
 - <csr-id-145ec301fff026ab46f57c62e3093403f0055963/> integrate to the system
 - <csr-id-186f49374e1897d7ddfc05499783d717a89704cd/> implement an in-memory Register storage
 - <csr-id-e6bb10ea9d5e829826520384fbfc3a6c61f7c494/> implement an in-memory Chunk storage
 - <csr-id-7543586c0ad461c54bce95458660d6e2b7ee9492/> add a basic level of churn to nodes
   restarting them at random even in small networks
 - <csr-id-5ce1e89c56cebd9c61f8032c2ca86c258e5f033a/> make req/resp generic
 - <csr-id-a77b33df2a846423eabf8debfcf15f0ac50f085d/> implement req/resp to store and retrieve chunks
 - <csr-id-bbe5dce01ab88e33caf9106338506ec98aa48387/> properly handle joined nodes before sync
 - <csr-id-bd396cf46e5d1a55dc74cc18412e5b8816df05b5/> some joining, but not enough sync
 - <csr-id-02e3ee80fde50d909984e5b80b6b0300d42367bb/> accept sync msg, update valid comm targets
 - <csr-id-8c34f90a7ad3c3670b415b9845aac46488a50965/> send sync msg after handling
 - <csr-id-1b92b346f07aee6b92f782a66257b148dcb45785/> start sending joins
 - <csr-id-514e8153bfc33cd5bb12e7998dd065e5f5c30c4c/> add some logging to dirs per node
 - <csr-id-e7f1da121e9b7afd2784caeab1fd8b826c47fa85/> use a random port @ startup, write config if none exists
 - <csr-id-fa4b3eacb4930749ad229cf2dbd26949b0a77a7e/> initial copy of testnet bin with basic tweaks.

### Bug Fixes

 - <csr-id-e6d0c27766a12ae9803a8e050003ae2e4bb77e88/> using different identify for client
 - <csr-id-35f835a7726c7a4a7e75b63294834e7beffb3b69/> confirm network connected within client constructor
 - <csr-id-e27dc6bcb9da5f277880e485ce4438f1cfde6c66/> use tokio::select to await two futures on event loop
 - <csr-id-9c6a724185abe970b966597b1355c04089b4e632/> avoid stall among select
 - <csr-id-478f7a64a1e0d4642a2380f160a22dc3e38568ca/> avoid deadlock during spend dbc
 - <csr-id-12e66c9f052c6d4e810c72e8e68e1fd78ea120b2/> some register TODOs
 - <csr-id-3ee6bd1d287c6e1f5305b478eebae97c9328d5e8/> do not error on popped add fail
   - Adding the spend popped from priority queue shall not error back to
   the sender who sent in a spend, since it is not related, i.e. the popped
   spend is very likely a completely different spend.
 - <csr-id-ad57f918416556d7c92be2d830d6aefdc89f73bb/> rem validate spend on filepath exists
   - This validation doesn't make sense, as we've gone through it multiple
   times already at that stage.
 - <csr-id-a335cedbbdd53264de542d174faa44589eb9ead5/> use println instead of print
   - Wrong macro used by mistake.
 - <csr-id-d06de2f8fe59d922afe9ed542bd49b45efa0e9a2/> make cli output usable again
 - <csr-id-ba188695fde79c9da5ca5bf63126986bc6bbb811/> store faucet state before verifying
 - <csr-id-6696f952f875f1297320f41dfc6751ea87691382/> post rebase issue
 - <csr-id-04e7933affd48a2bf7eea58abffccbd0629ff02e/> temp disable doublespend detection
   - This is a temp fix to network issues, to be enabled again asap.
 - <csr-id-b65159627ff81ef67bef9ac7b16558a571d3047f/> keep the event_rx task running
 - <csr-id-2da4e97fa8bfb036d1dbd1e04e8679ef53920201/> initialize logging once for unit tests
 - <csr-id-e0562349b5cd62471ead756daeb919887adae0be/> remove timeout from `send_to_closest`
   - The request_response Behaviour contains an inbuilt timeout. Hence
     remove our custom timeout implementation.
 - <csr-id-9f5596b1d1a30d75be67ba68b6c6a6a9d4ffb79d/> get our `PeerId` from Network
 - <csr-id-fe39d932837a74dac973d0ca7c230bce45fef5dd/> init logger for client executables
 - <csr-id-5b07522a341dc9830ebcf14b29244217c5833df6/> terminate on get record failure
 - <csr-id-bc6ef608a5379ac64a04289b5d4ab14b0cfb120c/> make client cfg consistent with node
 - <csr-id-0f905452f6c2f081eb7d214f08668e5b1dd4a10c/> the test should transfer half amount
   - This makes validation of resulting balance at sender simpler.
 - <csr-id-3a60906779f306a79cba1aa7faf6e15bc584a8b5/> correctly state majority and not all needed
 - <csr-id-50321d1dac0fcb2bc79108f2ed37f86076e9d579/> remove bughunt temp allow of single response
 - <csr-id-8b621f87eee9aca07d0b48734f71fe0684734271/> validate correct tx
   - The tx where fee payment is found is that in the signed spend, not in
   the parent.
 - <csr-id-c6f5713e8ab640806abf70ce2117468d75943a5a/> account for all fees in transfer to faucet
 - <csr-id-6c5fec3e880afbf3633b770db3698c718fdb1ea7/> store chunk as kad record directly
 - <csr-id-df0dc757b307d5d6153bed2292b52c1c076c8834/> do not verify small chunks before attempting to upload
 - <csr-id-18241f6b280f460812acd743b601ad3c4cce5212/> add root dir to node startup
 - <csr-id-aecde8e92a1992956e7a41d8d98628e358a7db75/> remove txt extension
   - Since we store serialized data to the file, the `plain text document`
   file extension is misleading.
 - <csr-id-10ff6c70e1211e6a00387170158cb7ada7c43071/> use correct name for downloaded files
   - Stores the file names to the xorname index doc, so that the downloaded
   files can get their proper file names.
 - <csr-id-25471d8c941aa20e60df8b17d82f0a36e3e11fba/> do not panic in cli
   - There is no need for it. Print what did/did not happen and exit.
 - <csr-id-1f7150b56ccee91c3b405e391f151320cf150fc1/> do not error if remove failed
   - When adding reported double spend, we might not have a valid spend
   stored, and thus we should not error if it wasn't found when we try
   to remove it.
 - <csr-id-47a0712c0ba475f240612d0918d1ab5a12ba45cf/> properly generate reg cmd id
 - <csr-id-6bc5ec704b54063ab923010c9d826905a7aa9c88/> incorrect slice copying
 - <csr-id-99d980251523e03efe415f348ac4d6017aeed67c/> get register id without serializing
 - <csr-id-1202626802b2a9d06ba4274d0b475714c8375267/> proper path for client upload and download tests
 - <csr-id-8651c5ed482475c5c53ae5e74ff68078dbed36c2/> add missing tracing macro to client
 - <csr-id-8d4c5f5a466b59ae5d14252a3c3fe229a123ec55/> resolve error due to client API change
 - <csr-id-42f021b0974a275e1184131cb6621cb0041454e7/> doc references
 - <csr-id-624ac902974d9727acea10ed1d2a1a5a7895abb9/> reduce conflict resolve in rebase
 - <csr-id-8cd5a96a0ce4bea00fe760c393518d684d7bbbcc/> make rpc urls https
   This should allay devskim fears
 - <csr-id-39b82e2879b95a6ce7ded6bc7fc0690d2398f27c/> use hash of PeerId to calculate xorname instead of chopping bytes
 - <csr-id-c3d7e4a6780e8d010ca4d9f05908155df77124d2/> lower mdns query interval for client stability
 - <csr-id-e31e4d34bf75129514218c9ff4ceeed1b84651c3/> add additional layer of race prevention
   - Added in case some oblivious developer in the future removes `&mut`
   self from the fn signature.
 - <csr-id-00cce808950c5eb0a346ecf07b3a9d40dbfc88de/> add &mut self to transfers fn signatures
   - This is necessary to avoid race conditions when checking for double
   spends.
 - <csr-id-17daccbd2b42acd1b9727ffa5b4e2e8f0df9142c/> select majority of same spends
   - This fixes the previous implementation where a single rogue node could
   prevent the conclusion of a valid spend when requesting it from the
   close group.
 - <csr-id-a41bc935855112bc129d81fdac4f75667088d757/> vanishing outputs #92
 - <csr-id-c496216ee15e97a110e30851c42144376676b045/> make wallet pass sending test
   - Sending decreases balance and produces a correct output dbc.
 - <csr-id-6040e2d2be6a8198d5cae73f70e7d815262f3352/> client should not be present inside closest_peers
 - <csr-id-9f342492dc702656f961991f9e3e5ec991c94e90/> avoid lost spawned handler
 - <csr-id-ac488dbcafcf5f999f990eaf156bedf15213570c/> correct termination of get_closest_peers
 - <csr-id-2c3657d58884acd239d82e3099052a970fad8493/> use the closest nodes to put/get data
 - <csr-id-892c8b3abf332fbbe100bf04c0b04cc9e67be828/> add env filter and strip back testnet bin
 - <csr-id-500566d66c08aa89ccd2a0ad43ef99b5d83ce5c3/> use Error enum
 - <csr-id-c6ae34f3a8abb5657e08b234e9f1810ee1435ec1/> use libp2p-quic instead of the quic feature
 - <csr-id-5e633868773e42c13326c2f52790c94d4cd88ae0/> clippy lints
 - <csr-id-63081bc27b6f6d3280ad3e55dddf934177368569/> enable log level through env variable
 - <csr-id-6190d222e04904baad12070f3893c2d0c425238a/> initial comms by writing 127.0.0.1 ip addre for genesis

### Refactor

 - <csr-id-92fd989c55b870713c97d3932efbf99325b0dcbf/> add strings as const
 - <csr-id-1d8b9fae18fa1502c9000dce4cd4400cdf301cb5/> restructuring protocol Error types and removing unnecessary variants
 - <csr-id-8c093c40cbdbc9e28791dcb3d47e87ee8fc0da37/> removing helpers from protocol types
 - <csr-id-026fb6de3c38bd61d5438869822ebb2cbcf5f9e6/> do not put spend to queue if stored
 - <csr-id-3f1fe909ee5515b13dfaa89cb87999d71ae95d9e/> temp disable transfer rate limit
   - This will be enabled again when transfers feat have stabilized.
 - <csr-id-abd891cbec2250b7263dfe9e582bb2cd82f70cec/> use add order aware pending txs list
   - This is crucial for making the wallet state usable, so that spends
   that rely on earlier spends, don't fail because the earlier ones are
   not yet in.
 - <csr-id-435cca51ad8164a131a5ba7911272aa819e53d3c/> update client wallet state before send
 - <csr-id-a60ad2338190b4ca6d1341ea41bc1f266aea0810/> parallelize spend requests
 - <csr-id-575c9e5569c55ad7bac24c1e3e49047a79d716b7/> parallelise client verif network calls
 - <csr-id-6c4e0d04d1d39a8fe7807c38750029eb1807e4fa/> remove node init with genesis spend
 - <csr-id-abb29c4116a1622377ade80539becf86b7369dd8/> move faucet creation to dbc genesis file
 - <csr-id-5bdd2a78aa96f1d33cf53b907a3c4c2b20a07010/> genesis error
   - Remove type aliasing in genesis module.
 - <csr-id-fc09d93193756798bd0be5d9375045e00c7a2295/> initialize node api with genesis
 - <csr-id-6d5856c7056e66f0efe6e50b64032a4d1b0bc24e/> init transfers with node wallet
 - <csr-id-0c495d7ff2175969ffb31faf3dd29b031c5252ab/> move out signing from required fee
 - <csr-id-a19759bc635fbda2d64bc8bcc874345c6bcca14c/> assert_fs instead of temp_dir in tests
 - <csr-id-e961f281a9854845d3ca7028a3b9856bee8f73e4/> move non-protocol related code to domain
   - This structures the project code after well known practices where the
   protocol is the rules and conventions that govern how data is
   transmitted and communicated over the network, and the domain refers to
   the subject area and problem space that the softweare is designed to
   address. It represents the business logic, processes, and rules
   associated with the specific features and services of the system.
 - <csr-id-e6101a5ef537e1d56722bab86c7fd45c9d964bc9/> implement storage error
 - <csr-id-1e63801d2e3dcfa3aeb27cb3cbdc6e46468a44cb/> remove used space
   - This is the first step in removing the limitation on storage size.
   The nodes will be allowed to store as much as they can, and later
   offload excess to archive nodes. If they run out of space that will be
   identified by fault detection and they will be removed from lists.
 - <csr-id-8ebe87e140fbc7c3db47288f2f5a31ee283e488a/> move log dir param one level up
 - <csr-id-826bb0a646a9b69df0f62a4410108c8c9a3b7926/> use subcmds
   - There are some changes to the use of files and registers as well.
 - <csr-id-728dc69c1a4ef75a96552984b6428bbbec226696/> error on cli invalid amount
   - If sending tokens and an address has been specified, we will error if the
   amount can't be parsed.
 - <csr-id-b61dfa0a5a2f5051d7613d28760e3a37f176e0f8/> move node transfer logic to protocol
   - This keeps the logic levels more consistent
   - As client handling of transfers was already in protocol, it seems
   more stringent to also keep the node handling of transfers there.
 - <csr-id-56672e3c7d91053f2c3b37c24dc1cbac54c9e2e4/> use online transfer in client
   - This wires the client to use the online transfer flow, with fees.
   - This also merges the two online/offline mods into one transfer mod.
 - <csr-id-60e2f2961e1fa08d5700039fa362755a68143ebf/> remove invalid spend broadcasts
   - The only unspendable marking and broadcast we'll do is for detected
   double spend attempts.
   - We error back to client on other types of invalid spends or parents,
   and drop those spends.
 - <csr-id-48e04652f5ddd66f43f87455b4cf884c23bc96e6/> unify membership and stable_set
 - <csr-id-69bc68dad31ef2169398bf3a00c77422f8c33334/> share->witness & break up some methods
 - <csr-id-e17a1890d3254abc5e258cf662bfd79e71080949/> rename get_config
 - <csr-id-c5831ace461627781066ff2f8a75feda524f2ca7/> set socket addr by argument

### Test

 - <csr-id-8f459eb053c0e001a4fbdd7fe2c637c2289891bf/> improve transfer e2e test log
 - <csr-id-78f29f72488115670c576aa055d10e69447d6e33/> rename transfer e2e test
 - <csr-id-20af2bc156650e2fd39851ba0827efd0f15d91de/> ignore the double spend punishment test
 - <csr-id-aa8876098babf9252348e034e3b49b9803027018/> fix msg_to_self_should_not_error_out
   - Some retrying makes it pass every time.
 - <csr-id-52883b6b576c73862ab8acd78578f12feabf7297/> add deposit_is_idempotent test
 - <csr-id-9909a4474bb32987d70a02722a0692260d00c7f2/> modify transferred amounts
 - <csr-id-faf092c7b78039aff07f2edc09fcfdbab1eb49bc/> impl spend_is_stored_in_network test
 - <csr-id-cda0bc68c731d81cd419aa3cea88e62941f09ecd/> add created_dbc_to_file_can_be_deposited
 - <csr-id-bd7238bed980a57a163cdf8b543862c6614c0c91/> add try_add_fails_after_added_double_spend
 - <csr-id-332912f69f9046925fd2f64ab21b1f24c2a4a2bd/> add try_add_double_is_idempotent
 - <csr-id-49e81ec04257dd2787f07480c92427831bc13687/> add double_spend_attempt_is_detected
 - <csr-id-e0ff76db5cd390eefd6e1a3d3b997264ad454df6/> add adding_spend_is_idempotent
 - <csr-id-fc36acac9cea22531916f670ecc2acb53a5f6ea5/> add write_and_read_100_spends test
 - <csr-id-3fc4f20e1e6f7a5efa1aba660aed98297fe02df4/> client CLI confirming dead node gone in closest
 - <csr-id-6ad903878c797fc49c85f80bcd56278bbebee434/> network CI tests involves client actions
 - <csr-id-24bf65976123eba764f5b3193f1e09a92412a135/> validate closest peers

### Chore (BREAKING)

 - <csr-id-3bc834a3447d0bf1e1412135105c3db0e6c90071/> simplify faucet cli

### Bug Fixes (BREAKING)

 - <csr-id-08e2479d752f23c0343219c88287d6ae4c550473/> replace generic Error types with more specific ones

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 352 commits contributed to the release over the course of 41 calendar days.
 - 334 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Using different identify for client ([`e6d0c27`](https://github.com/maidsafe/safe_network/commit/e6d0c27766a12ae9803a8e050003ae2e4bb77e88))
    - Confirm network connected within client constructor ([`35f835a`](https://github.com/maidsafe/safe_network/commit/35f835a7726c7a4a7e75b63294834e7beffb3b69))
    - Code cleanup ([`3cff43c`](https://github.com/maidsafe/safe_network/commit/3cff43ca4fce96055a4f506425f7b1af76057188))
    - Use tokio::select to await two futures on event loop ([`e27dc6b`](https://github.com/maidsafe/safe_network/commit/e27dc6bcb9da5f277880e485ce4438f1cfde6c66))
    - Avoid stall among select ([`9c6a724`](https://github.com/maidsafe/safe_network/commit/9c6a724185abe970b966597b1355c04089b4e632))
    - Avoid deadlock during spend dbc ([`478f7a6`](https://github.com/maidsafe/safe_network/commit/478f7a64a1e0d4642a2380f160a22dc3e38568ca))
    - Write client logs to tmp dir by default. ([`85b359b`](https://github.com/maidsafe/safe_network/commit/85b359b686facefd65c56be1d54ca5ef0a9f10f6))
    - Add passed in peer to routing table ([`d7e344d`](https://github.com/maidsafe/safe_network/commit/d7e344df6aaca3bef75d7c9d90edca7d39771194))
    - Add strings as const ([`92fd989`](https://github.com/maidsafe/safe_network/commit/92fd989c55b870713c97d3932efbf99325b0dcbf))
    - Add identify behaviour ([`5693525`](https://github.com/maidsafe/safe_network/commit/5693525b2cb4c285fd80137ce1528ab6f2a69358))
    - Some register TODOs ([`12e66c9`](https://github.com/maidsafe/safe_network/commit/12e66c9f052c6d4e810c72e8e68e1fd78ea120b2))
    - Add AlreadyDialingPeer as error ([`fdeb508`](https://github.com/maidsafe/safe_network/commit/fdeb5086a70581abc4beb05914dd87b8ed791ffb))
    - Restructuring protocol Error types and removing unnecessary variants ([`1d8b9fa`](https://github.com/maidsafe/safe_network/commit/1d8b9fae18fa1502c9000dce4cd4400cdf301cb5))
    - Fixing doc dep ([`ecebc7a`](https://github.com/maidsafe/safe_network/commit/ecebc7a93b014aac397eca3d5149e5583e8be04f))
    - Moving all transfer fees related types onto the protocol crate/mode ([`5bd7fb9`](https://github.com/maidsafe/safe_network/commit/5bd7fb9f486fc85af8dfbc155e6435415b152c10))
    - Removing helpers from protocol types ([`8c093c4`](https://github.com/maidsafe/safe_network/commit/8c093c40cbdbc9e28791dcb3d47e87ee8fc0da37))
    - Isolating all 'protocol' types from their implementations ([`bab05b0`](https://github.com/maidsafe/safe_network/commit/bab05b011c8e5ecf70d2a6c61d9289eebc78f533))
    - Allow clients to dial specific network peers ([`e3b55c9`](https://github.com/maidsafe/safe_network/commit/e3b55c9cbbdf26e3018b736d29a31887c5355811))
    - Fix doc typos ([`f1d8be8`](https://github.com/maidsafe/safe_network/commit/f1d8be8cd15506df0432da9179f1de0b1c0b8f67))
    - Fire and forget broadcast of valid spend ([`2507695`](https://github.com/maidsafe/safe_network/commit/2507695a7af51de32d40ab90981975e0372916c3))
    - Impl fire and forget request in network ([`ee0d5fe`](https://github.com/maidsafe/safe_network/commit/ee0d5feedbfe80c513520ff6a9d914815b8087ee))
    - Improve logging for parent validation ([`6a53c1e`](https://github.com/maidsafe/safe_network/commit/6a53c1e7b54c7e93e485510556c7d8dd6a0eec3b))
    - Do not put spend to queue if stored ([`026fb6d`](https://github.com/maidsafe/safe_network/commit/026fb6de3c38bd61d5438869822ebb2cbcf5f9e6))
    - Impl spend store exists ([`772b972`](https://github.com/maidsafe/safe_network/commit/772b97208b7c756b1ecc25377e80d9d53baceff4))
    - Do not error on popped add fail ([`3ee6bd1`](https://github.com/maidsafe/safe_network/commit/3ee6bd1d287c6e1f5305b478eebae97c9328d5e8))
    - Rem validate spend on filepath exists ([`ad57f91`](https://github.com/maidsafe/safe_network/commit/ad57f918416556d7c92be2d830d6aefdc89f73bb))
    - Temp disable transfer rate limit ([`3f1fe90`](https://github.com/maidsafe/safe_network/commit/3f1fe909ee5515b13dfaa89cb87999d71ae95d9e))
    - Improve spend storage logs ([`0487be4`](https://github.com/maidsafe/safe_network/commit/0487be41aeeb96d0945e2b76a0045e3b19ffcf17))
    - Fix incorrect log msgs ([`fbee86d`](https://github.com/maidsafe/safe_network/commit/fbee86db94bf77cbf27a28f803f04005c5ac51cd))
    - Improve msg docs ([`e727fec`](https://github.com/maidsafe/safe_network/commit/e727feca3eb4626d05d4989e38366a4376dde127))
    - Improve transfer e2e test log ([`8f459eb`](https://github.com/maidsafe/safe_network/commit/8f459eb053c0e001a4fbdd7fe2c637c2289891bf))
    - Rename transfer e2e test ([`78f29f7`](https://github.com/maidsafe/safe_network/commit/78f29f72488115670c576aa055d10e69447d6e33))
    - Disable logging in spend e2e test ([`030fc25`](https://github.com/maidsafe/safe_network/commit/030fc25c8d9fa5e54f6937844cd6a633aff173cd))
    - Remove wait before verifying tranfer ([`31b7f66`](https://github.com/maidsafe/safe_network/commit/31b7f668a80f026ee768c9738282cc81dcb3f00b))
    - Node to broadcast storage events for Chunks, Registers and Spends ([`035c21b`](https://github.com/maidsafe/safe_network/commit/035c21b93ec8f03a2fa9d581a57d4a4a9bc9c707))
    - Use println instead of print ([`a335ced`](https://github.com/maidsafe/safe_network/commit/a335cedbbdd53264de542d174faa44589eb9ead5))
    - Traces to println for wallet file ([`70742c2`](https://github.com/maidsafe/safe_network/commit/70742c272fa8a92cdb3b15a14b803ee993e14aa9))
    - Traces to println for wallet file ([`4de0b10`](https://github.com/maidsafe/safe_network/commit/4de0b10e4f5a063427e4296c96e90e2f966bd621))
    - Traces to println for wallet localstore ([`3860a81`](https://github.com/maidsafe/safe_network/commit/3860a813ad543d7c7c436205453d90c484f1d4f1))
    - Change traces to println for register cli ([`ad182d8`](https://github.com/maidsafe/safe_network/commit/ad182d8d9e103e662a70d67c316a3a8fbe2b42f2))
    - Change traces to println for files cli ([`dae3ac5`](https://github.com/maidsafe/safe_network/commit/dae3ac55b164fc5ea73458a53728564bee6d03b2))
    - Change traces to println for wallet cli ([`3fe5943`](https://github.com/maidsafe/safe_network/commit/3fe59434384448a5d9c5b934710db45aabb3e22a))
    - Make cli output usable again ([`d06de2f`](https://github.com/maidsafe/safe_network/commit/d06de2f8fe59d922afe9ed542bd49b45efa0e9a2))
    - Store faucet state before verifying ([`ba18869`](https://github.com/maidsafe/safe_network/commit/ba188695fde79c9da5ca5bf63126986bc6bbb811))
    - Post rebase issue ([`6696f95`](https://github.com/maidsafe/safe_network/commit/6696f952f875f1297320f41dfc6751ea87691382))
    - Ignore the double spend punishment test ([`20af2bc`](https://github.com/maidsafe/safe_network/commit/20af2bc156650e2fd39851ba0827efd0f15d91de))
    - Typo fix ([`ff72c32`](https://github.com/maidsafe/safe_network/commit/ff72c32023b46df0a0f320f6b5480939da9b40b2))
    - Broadcast spend as node confirm its validity ([`c2ef1f6`](https://github.com/maidsafe/safe_network/commit/c2ef1f6d5defc075f80dfc0d0f6d6aec9d511d32))
    - Temp disable doublespend detection ([`04e7933`](https://github.com/maidsafe/safe_network/commit/04e7933affd48a2bf7eea58abffccbd0629ff02e))
    - Clarify current state of tx queue docs ([`36a92f7`](https://github.com/maidsafe/safe_network/commit/36a92f7dc4c9b4e97a1f45b755cde764af536305))
    - Remove unnecessary double store of hash ([`35846da`](https://github.com/maidsafe/safe_network/commit/35846da7e59d2f2c6cdef8538b813d19cac21680))
    - Replace println with trace in local wallet ([`f61119e`](https://github.com/maidsafe/safe_network/commit/f61119e255828a6222398db470e74aee8ad88d3e))
    - Use the error in local wallet send ([`d0d536d`](https://github.com/maidsafe/safe_network/commit/d0d536d6e66e43766fdb009cfe8672f738a986a9))
    - Increase comment line width ([`ddd438c`](https://github.com/maidsafe/safe_network/commit/ddd438c66e2b5fa71ea2b0d1e57d732af4deb447))
    - Document pending txs ([`23a309d`](https://github.com/maidsafe/safe_network/commit/23a309d1d3e2c1c6b928cfd7c2ebda9423798e77))
    - Resend pending txs when other transfer made ([`d95bf6c`](https://github.com/maidsafe/safe_network/commit/d95bf6cdbd10f907112bf2f707bbf0d2f7f8f235))
    - Add a failing spend to the pending txs list ([`3fc3332`](https://github.com/maidsafe/safe_network/commit/3fc3332e74323f4c635a89527075d9b6c61abcc5))
    - Use add order aware pending txs list ([`abd891c`](https://github.com/maidsafe/safe_network/commit/abd891cbec2250b7263dfe9e582bb2cd82f70cec))
    - Update wallet docs ([`6ab6f6a`](https://github.com/maidsafe/safe_network/commit/6ab6f6ab05a18ba4b00c5799c1ecf8a880426cb6))
    - Update client wallet state before send ([`435cca5`](https://github.com/maidsafe/safe_network/commit/435cca51ad8164a131a5ba7911272aa819e53d3c))
    - Minor style fixes ([`00248dd`](https://github.com/maidsafe/safe_network/commit/00248dd8264ac75f6967be19ebd9f34ad7ebfdcd))
    - Fix msg_to_self_should_not_error_out ([`aa88760`](https://github.com/maidsafe/safe_network/commit/aa8876098babf9252348e034e3b49b9803027018))
    - Keep the event_rx task running ([`b651596`](https://github.com/maidsafe/safe_network/commit/b65159627ff81ef67bef9ac7b16558a571d3047f))
    - Initialize logging once for unit tests ([`2da4e97`](https://github.com/maidsafe/safe_network/commit/2da4e97fa8bfb036d1dbd1e04e8679ef53920201))
    - Fix(network): route `Request` and `Response` to self - While using the `RequestResponse` behaviour, we get a `OutboundFailure::DialFailure` if we try to send a request to `self` - So if `self` is the recipient of the `Request`, then route the request   directly to `self` without using the `RequestResponse` behaviour. - This request, then follows the normal flow without having any custom   branch on the upper layers. The produced `Response` is also routed   back to `self` ([`1510e5f`](https://github.com/maidsafe/safe_network/commit/1510e5fc8730ada889b4451d2205e16e1c5ddd34))
    - Set timeout through the `RequestResponse` behaviour ([`17849dc`](https://github.com/maidsafe/safe_network/commit/17849dcbbc8bea681a3d78a62ba7613877eab81a))
    - Remove timeout from `send_to_closest` ([`e056234`](https://github.com/maidsafe/safe_network/commit/e0562349b5cd62471ead756daeb919887adae0be))
    - Get our `PeerId` from Network ([`9f5596b`](https://github.com/maidsafe/safe_network/commit/9f5596b1d1a30d75be67ba68b6c6a6a9d4ffb79d))
    - Disable some very verbose log outputs ([`6979d05`](https://github.com/maidsafe/safe_network/commit/6979d05e5574163b47d6184d217c993a1c72ee3d))
    - Fast refresh delays ([`903c59f`](https://github.com/maidsafe/safe_network/commit/903c59f09f8520dad129fcf97685877b0bfe78f7))
    - Init logger for client executables ([`fe39d93`](https://github.com/maidsafe/safe_network/commit/fe39d932837a74dac973d0ca7c230bce45fef5dd))
    - Simplify faucet cli dir structure ([`5b28b75`](https://github.com/maidsafe/safe_network/commit/5b28b75e8f65ff2f4ea33fec7c63e813a64c3c4d))
    - Simplify faucet cli ([`3bc834a`](https://github.com/maidsafe/safe_network/commit/3bc834a3447d0bf1e1412135105c3db0e6c90071))
    - Add client cli cmd balance ([`ad7de37`](https://github.com/maidsafe/safe_network/commit/ad7de377a0aa0e47c09778ed1f2951a77e5eed90))
    - Add client cli cmd address ([`e5bf209`](https://github.com/maidsafe/safe_network/commit/e5bf209b5c1bcea0a114f32a1737bb0b4101d5c7))
    - Remove limit-client-upload-size ([`360cd85`](https://github.com/maidsafe/safe_network/commit/360cd85cd0c3ce2acad5438a22cea1a2650de3f8))
    - Terminate on get record failure ([`5b07522`](https://github.com/maidsafe/safe_network/commit/5b07522a341dc9830ebcf14b29244217c5833df6))
    - Make client cfg consistent with node ([`bc6ef60`](https://github.com/maidsafe/safe_network/commit/bc6ef608a5379ac64a04289b5d4ab14b0cfb120c))
    - Rename RequestResponse to MsgReceived ([`396b3e9`](https://github.com/maidsafe/safe_network/commit/396b3e9f06a8d76af521552a5ffe1eb7eb57078b))
    - Skip get spend debug due to very verbose ([`63806e3`](https://github.com/maidsafe/safe_network/commit/63806e3d95bdcfbf97e00bb57eb93ff6c8c092fb))
    - Impl early return for majority get spend ([`1513ef5`](https://github.com/maidsafe/safe_network/commit/1513ef5f33993cc417e969d36ca50055884f10ea))
    - Improve fn naming ([`7880e41`](https://github.com/maidsafe/safe_network/commit/7880e416140c10600ff9f35fb4b1ad195de336c8))
    - Parallelize spend requests ([`a60ad23`](https://github.com/maidsafe/safe_network/commit/a60ad2338190b4ca6d1341ea41bc1f266aea0810))
    - Remove params suffix from struct name ([`79174f4`](https://github.com/maidsafe/safe_network/commit/79174f45852add379610480301dd8ad888dbb164))
    - Impl early return for majority spend ok ([`a71b9ff`](https://github.com/maidsafe/safe_network/commit/a71b9ffca53a7e5a7e1a75f38c00c4a59c8acbae))
    - The test should transfer half amount ([`0f90545`](https://github.com/maidsafe/safe_network/commit/0f905452f6c2f081eb7d214f08668e5b1dd4a10c))
    - Parallelise client verif network calls ([`575c9e5`](https://github.com/maidsafe/safe_network/commit/575c9e5569c55ad7bac24c1e3e49047a79d716b7))
    - Correctly state majority and not all needed ([`3a60906`](https://github.com/maidsafe/safe_network/commit/3a60906779f306a79cba1aa7faf6e15bc584a8b5))
    - Move logic in block to fn and clarify docs ([`d049172`](https://github.com/maidsafe/safe_network/commit/d049172fff516df5d22b4a32e74cfe828704ac4d))
    - Move client transfer traits to own files ([`d7807fe`](https://github.com/maidsafe/safe_network/commit/d7807febbadf891e320c5a265743e14d698086d5))
    - Remove unnecessary allow that snuck in ([`a2f054a`](https://github.com/maidsafe/safe_network/commit/a2f054a3d0deb560cfea2208fcea0d1af8cc55f8))
    - Remove bughunt temp allow of single response ([`50321d1`](https://github.com/maidsafe/safe_network/commit/50321d1dac0fcb2bc79108f2ed37f86076e9d579))
    - Add deposit_is_idempotent test ([`52883b6`](https://github.com/maidsafe/safe_network/commit/52883b6b576c73862ab8acd78578f12feabf7297))
    - Fix copy paste doc error ([`b51a8b0`](https://github.com/maidsafe/safe_network/commit/b51a8b04d3a99af93714da9f68f12c360176ce1c))
    - Identify genesis spend ([`cab992f`](https://github.com/maidsafe/safe_network/commit/cab992f23070894107696a20de12d94e7a381dea))
    - Load genesis dbc from hex ([`8270cdb`](https://github.com/maidsafe/safe_network/commit/8270cdb96888bdf35f896ec0ce4ff9a27a6d6274))
    - Validate correct tx ([`8b621f8`](https://github.com/maidsafe/safe_network/commit/8b621f87eee9aca07d0b48734f71fe0684734271))
    - Move temp allow large err to one place ([`d774fb8`](https://github.com/maidsafe/safe_network/commit/d774fb80860f2747e583fc511a8d84e6a5cde237))
    - Differentiate missing fee errors ([`8bf5d57`](https://github.com/maidsafe/safe_network/commit/8bf5d578bec4d72dac1c412c2b2d456cd9f4e212))
    - Use print instead of log in client api ([`f81b5a3`](https://github.com/maidsafe/safe_network/commit/f81b5a34a0c166a0dbd91618205a1a61bc1aa87a))
    - Impl display for data address ([`f8c2975`](https://github.com/maidsafe/safe_network/commit/f8c29751ffcaecb3401715dd0f5a6d87f5e70146))
    - Impl display for cmd ([`8a43ddf`](https://github.com/maidsafe/safe_network/commit/8a43ddfe28408e032b481fb8d88c1234df17be5e))
    - Impl display for query ([`e0ee848`](https://github.com/maidsafe/safe_network/commit/e0ee848017cd41a66bad18e1004644e982f7e41e))
    - Impl display for spendquery ([`2b9bb9f`](https://github.com/maidsafe/safe_network/commit/2b9bb9fc0052cb68801973aa342ab8ec6bfc2241))
    - Remove node init with genesis spend ([`6c4e0d0`](https://github.com/maidsafe/safe_network/commit/6c4e0d04d1d39a8fe7807c38750029eb1807e4fa))
    - Log individual spend errors at client ([`7151474`](https://github.com/maidsafe/safe_network/commit/71514749883e62c90d0ecfacf371499c8373d054))
    - Improve error msg too few ok responses ([`688fe6b`](https://github.com/maidsafe/safe_network/commit/688fe6bbea6db783bae6c601cb6fbf05cc57d16c))
    - Impl display for response ([`1318073`](https://github.com/maidsafe/safe_network/commit/13180738c4ca1440a91cba7554208e1e0735c5ec))
    - Impl display for queryresponse ([`510b4cc`](https://github.com/maidsafe/safe_network/commit/510b4cc1d19c678f4c8ae984b5c5835662c69cda))
    - Modify transferred amounts ([`9909a44`](https://github.com/maidsafe/safe_network/commit/9909a4474bb32987d70a02722a0692260d00c7f2))
    - Account for all fees in transfer to faucet ([`c6f5713`](https://github.com/maidsafe/safe_network/commit/c6f5713e8ab640806abf70ce2117468d75943a5a))
    - Rename faucet cmd variant to ClaimGenesis ([`3bc906b`](https://github.com/maidsafe/safe_network/commit/3bc906b02dfeb18149d76c8e0d5f833c5a74a212))
    - Remove unnecessary file ([`114a54c`](https://github.com/maidsafe/safe_network/commit/114a54c8def8f131a22b810b9507f06a4bc3a13e))
    - Move faucet creation to dbc genesis file ([`abb29c4`](https://github.com/maidsafe/safe_network/commit/abb29c4116a1622377ade80539becf86b7369dd8))
    - Use deterministic blinding for genesis ([`abfd1a6`](https://github.com/maidsafe/safe_network/commit/abfd1a621bb00382549b1d4b93a815dfb9a2debf))
    - Rename dbc create fns ([`a63b259`](https://github.com/maidsafe/safe_network/commit/a63b2599bd49f6bcece4d55345a98379e11d59b6))
    - Verify a dbc is valid in the network ([`9590816`](https://github.com/maidsafe/safe_network/commit/959081620e1787accb4959bee6b01dfff7fe6024))
    - Verify close group majority ok a spend ([`b1d5f5c`](https://github.com/maidsafe/safe_network/commit/b1d5f5c5c0cbe07e0ec1c4ed801c617d059c5ed6))
    - Impl spend_is_stored_in_network test ([`faf092c`](https://github.com/maidsafe/safe_network/commit/faf092c7b78039aff07f2edc09fcfdbab1eb49bc))
    - Add setup for transfer e2e tests ([`5f6aace`](https://github.com/maidsafe/safe_network/commit/5f6aace3c14160b616fe705f2998cc161300bffb))
    - Impl verification of deposited dbc ([`b0d9d45`](https://github.com/maidsafe/safe_network/commit/b0d9d4521bc1c05b21fc659a593be7369a94574d))
    - Store chunk as kad record directly ([`6c5fec3`](https://github.com/maidsafe/safe_network/commit/6c5fec3e880afbf3633b770db3698c718fdb1ea7))
    - Rewording / renaming refactors for chunk provider setup ([`dbe2165`](https://github.com/maidsafe/safe_network/commit/dbe2165f05dce1c65b42835eb3763e725cf086a1))
    - Update MemoryStore cfg for larger record sizing ([`88223b7`](https://github.com/maidsafe/safe_network/commit/88223b77527c5645228a4a00cba4cd51e184fe06))
    - Remove chunk storage ([`fee76e8`](https://github.com/maidsafe/safe_network/commit/fee76e8650647b32dc4bd4ee95e2205398f4e04e))
    - Do not verify small chunks before attempting to upload ([`df0dc75`](https://github.com/maidsafe/safe_network/commit/df0dc757b307d5d6153bed2292b52c1c076c8834))
    - Use provider and MemoryStorte for retreiving chunks ([`55cef54`](https://github.com/maidsafe/safe_network/commit/55cef547a71b524e1bd1a17b98105bd6867de769))
    - Use kad MemoryStore for registering Providership of chunks ([`ddb8ea1`](https://github.com/maidsafe/safe_network/commit/ddb8ea170c5ead4988e9aecd8d21768f5dfe34b4))
    - Initial setup of KademliaConfig for provider usage ([`4eeeddc`](https://github.com/maidsafe/safe_network/commit/4eeeddc415cd625a898b7af8b6b19b7a6b91dfd2))
    - Example cmd querying network info from node's RPC service ([`16e6049`](https://github.com/maidsafe/safe_network/commit/16e60498965deb0b209429a50ca54016095f2879))
    - Exposing an RPC service to retrieve node connection info ([`66eeff3`](https://github.com/maidsafe/safe_network/commit/66eeff38da7cdcfd8b3e2230ca1e654d15cfd1e5))
    - Add root dir to node startup ([`18241f6`](https://github.com/maidsafe/safe_network/commit/18241f6b280f460812acd743b601ad3c4cce5212))
    - Fix doc typo ([`6fb46aa`](https://github.com/maidsafe/safe_network/commit/6fb46aae8acefbfa130d152aaabf6c429c9bf630))
    - Fix required fee ctor doc ([`ec859ec`](https://github.com/maidsafe/safe_network/commit/ec859ec379edc47718929a7e188590e0686b03b1))
    - Genesis error ([`5bdd2a7`](https://github.com/maidsafe/safe_network/commit/5bdd2a78aa96f1d33cf53b907a3c4c2b20a07010))
    - Initialize node api with genesis ([`fc09d93`](https://github.com/maidsafe/safe_network/commit/fc09d93193756798bd0be5d9375045e00c7a2295))
    - Cleanup transfer errors ([`fe86af5`](https://github.com/maidsafe/safe_network/commit/fe86af5632cce2639d36ce5b735efc8d70e301b9))
    - Init transfers with node wallet ([`6d5856c`](https://github.com/maidsafe/safe_network/commit/6d5856c7056e66f0efe6e50b64032a4d1b0bc24e))
    - Impl fee cipher decrypt for wallet ([`e9bfec3`](https://github.com/maidsafe/safe_network/commit/e9bfec3fcd300a714733a7718206797e5116d80d))
    - Move out signing from required fee ([`0c495d7`](https://github.com/maidsafe/safe_network/commit/0c495d7ff2175969ffb31faf3dd29b031c5252ab))
    - Impl new_with_genesis for Transfers ([`caac9e9`](https://github.com/maidsafe/safe_network/commit/caac9e99d0bc763ee3b6c3861ba4151bdcf947a7))
    - Impl new_with_genesis for SpendStorage ([`bb376bc`](https://github.com/maidsafe/safe_network/commit/bb376bcc1320d8477daab3ce3b76b08c090114e6))
    - Impl wallet sign ([`a17876e`](https://github.com/maidsafe/safe_network/commit/a17876e9190b4db6d4859736f569770827d0b2b1))
    - Update faucet mod docs with example ([`076cf55`](https://github.com/maidsafe/safe_network/commit/076cf5509a1afedbc416c37a67632abe972c168c))
    - Update incorrect cli cmd docs ([`044551d`](https://github.com/maidsafe/safe_network/commit/044551d5aa295d9e2bc3d2527ca969a32858cc2d))
    - Clarify test fn doc ([`fe4fa10`](https://github.com/maidsafe/safe_network/commit/fe4fa10c26f7e284a4806f19dfb915b6d105dceb))
    - Add faucet module docs ([`16e389d`](https://github.com/maidsafe/safe_network/commit/16e389da94aac51c46cc13c23ece1f54fa152ff9))
    - Remove unused files ([`08c65ff`](https://github.com/maidsafe/safe_network/commit/08c65ffc2b6d90ef843b21e157927bbb23406ec9))
    - Add testnet faucet to cli ([`044b05d`](https://github.com/maidsafe/safe_network/commit/044b05d34c5686076f9673c2cabbd76cd6902a37))
    - Rename kadclient to safe ([`3ee3319`](https://github.com/maidsafe/safe_network/commit/3ee3319d18dcd29b8d16c4ae24fbfad1be0e1e1c))
    - Move kadclient and its cli to own dir ([`f6e1c53`](https://github.com/maidsafe/safe_network/commit/f6e1c532171e72f52026195431cc0e836627f513))
    - Improve match clause ([`72c67ba`](https://github.com/maidsafe/safe_network/commit/72c67ba9199b3f105bd398cf34e0be88afedc5db))
    - Clarify test name ([`3db9b55`](https://github.com/maidsafe/safe_network/commit/3db9b55223bcfa6e81df0ec23d36b3b2f7d68d44))
    - Assert_fs instead of temp_dir in tests ([`a19759b`](https://github.com/maidsafe/safe_network/commit/a19759bc635fbda2d64bc8bcc874345c6bcca14c))
    - Make cli wallet cmd a bit less technical ([`504f4ee`](https://github.com/maidsafe/safe_network/commit/504f4ee5b10b75138044b1af8150825b53f776d3))
    - Clean up and add assertion to test ([`0559ca0`](https://github.com/maidsafe/safe_network/commit/0559ca06fb3d00e80e76d9736b030a543e34fc4c))
    - Create received_dbcs dir by default ([`04a724a`](https://github.com/maidsafe/safe_network/commit/04a724afbc9495937b8be7ab905f9695e68ad398))
    - Add created_dbc_to_file_can_be_deposited ([`cda0bc6`](https://github.com/maidsafe/safe_network/commit/cda0bc68c731d81cd419aa3cea88e62941f09ecd))
    - Update cli to not take path ([`fc895e3`](https://github.com/maidsafe/safe_network/commit/fc895e3577a94f620bf398b6cb3b2f189f34ebd0))
    - Store created dbcs as hex to file ([`705c67f`](https://github.com/maidsafe/safe_network/commit/705c67f672f4be870c4aae6b82c33f7cb7d0a89f))
    - Remove txt extension ([`aecde8e`](https://github.com/maidsafe/safe_network/commit/aecde8e92a1992956e7a41d8d98628e358a7db75))
    - Use correct name for downloaded files ([`10ff6c7`](https://github.com/maidsafe/safe_network/commit/10ff6c70e1211e6a00387170158cb7ada7c43071))
    - Allow downloading files to file system ([`71acb3c`](https://github.com/maidsafe/safe_network/commit/71acb3cc8383e4b8669c0c95cb302d05b1f8c904))
    - Move get client dir to kadclient ([`9e11748`](https://github.com/maidsafe/safe_network/commit/9e11748a191b4432499ceb6beded2a9dda15cf56))
    - Do not panic in cli ([`25471d8`](https://github.com/maidsafe/safe_network/commit/25471d8c941aa20e60df8b17d82f0a36e3e11fba))
    - Do not error if remove failed ([`1f7150b`](https://github.com/maidsafe/safe_network/commit/1f7150b56ccee91c3b405e391f151320cf150fc1))
    - Add try_add_fails_after_added_double_spend ([`bd7238b`](https://github.com/maidsafe/safe_network/commit/bd7238bed980a57a163cdf8b543862c6614c0c91))
    - Add try_add_double_is_idempotent ([`332912f`](https://github.com/maidsafe/safe_network/commit/332912f69f9046925fd2f64ab21b1f24c2a4a2bd))
    - Add double_spend_attempt_is_detected ([`49e81ec`](https://github.com/maidsafe/safe_network/commit/49e81ec04257dd2787f07480c92427831bc13687))
    - Add adding_spend_is_idempotent ([`e0ff76d`](https://github.com/maidsafe/safe_network/commit/e0ff76db5cd390eefd6e1a3d3b997264ad454df6))
    - Add write_and_read_100_spends test ([`fc36aca`](https://github.com/maidsafe/safe_network/commit/fc36acac9cea22531916f670ecc2acb53a5f6ea5))
    - Move missed domain logic ([`e9ce090`](https://github.com/maidsafe/safe_network/commit/e9ce090c2361dcd49400112f8d2e3d29386602d7))
    - Properly generate reg cmd id ([`47a0712`](https://github.com/maidsafe/safe_network/commit/47a0712c0ba475f240612d0918d1ab5a12ba45cf))
    - Additional review comment fixes ([`fb095b5`](https://github.com/maidsafe/safe_network/commit/fb095b5e63f826f4079ba2c7797a241969346d0b))
    - Apply fixes from review comments ([`dfe80b9`](https://github.com/maidsafe/safe_network/commit/dfe80b902f0e8f6803eb836aeb9c81363ae183a9))
    - Add missing asserts to reg tests ([`bc7bbb3`](https://github.com/maidsafe/safe_network/commit/bc7bbb3a502f2e5d2c673678e2f7bc132bc4b490))
    - Incorrect slice copying ([`6bc5ec7`](https://github.com/maidsafe/safe_network/commit/6bc5ec704b54063ab923010c9d826905a7aa9c88))
    - Remove unused dep ([`1b474d5`](https://github.com/maidsafe/safe_network/commit/1b474d5d5ca952dba9a785b31df6201a62c1b34e))
    - Minor comment fixes ([`69c1345`](https://github.com/maidsafe/safe_network/commit/69c13458a737221d75fccc73d8e534331d4dbe2e))
    - Spends drive storage ([`6916b4e`](https://github.com/maidsafe/safe_network/commit/6916b4e1af97c982a77a649be7889fcd0b4637e8))
    - Register drive storage ([`30586c9`](https://github.com/maidsafe/safe_network/commit/30586c9faa43489e7565164c768fa9afb3959e88))
    - Add missing comments and remove old ([`4fbddd2`](https://github.com/maidsafe/safe_network/commit/4fbddd23e174329dc97f8d66c387b5544366e620))
    - Get register id without serializing ([`99d9802`](https://github.com/maidsafe/safe_network/commit/99d980251523e03efe415f348ac4d6017aeed67c))
    - Remove unnecessary error mappings ([`435208c`](https://github.com/maidsafe/safe_network/commit/435208c7dc1c51e1d51f730c84ac648cff1026a1))
    - Chunk drive storage ([`1a8622c`](https://github.com/maidsafe/safe_network/commit/1a8622cb26db066481a9d12fce1065a1d57abcb4))
    - Proper path for client upload and download tests ([`1202626`](https://github.com/maidsafe/safe_network/commit/1202626802b2a9d06ba4274d0b475714c8375267))
    - Detect dead peer ([`69d1943`](https://github.com/maidsafe/safe_network/commit/69d1943d86870d08a9e1067a05b689af7e32711b))
    - Remove unused macro ([`66ba179`](https://github.com/maidsafe/safe_network/commit/66ba179061f5dcd13369edd7a569df9c0e1e5002))
    - Remove unused log line ([`e39363c`](https://github.com/maidsafe/safe_network/commit/e39363c8418e9e738c8e5380208666c20cbfed5d))
    - Add missing tracing macro to client ([`8651c5e`](https://github.com/maidsafe/safe_network/commit/8651c5ed482475c5c53ae5e74ff68078dbed36c2))
    - Resolve error due to client API change ([`8d4c5f5`](https://github.com/maidsafe/safe_network/commit/8d4c5f5a466b59ae5d14252a3c3fe229a123ec55))
    - Fix doc refs ([`05f5244`](https://github.com/maidsafe/safe_network/commit/05f5244afdd588ff71abcf414f3b81eb16803883))
    - Move non-protocol related code to domain ([`e961f28`](https://github.com/maidsafe/safe_network/commit/e961f281a9854845d3ca7028a3b9856bee8f73e4))
    - Remove file logs from client cli ([`b96904a`](https://github.com/maidsafe/safe_network/commit/b96904a5278ab1105fa4de69114151b61d0ada70))
    - Additional error variant cleanup ([`7806111`](https://github.com/maidsafe/safe_network/commit/78061111dc92f86ba976b8e75f49f02d3276d6d7))
    - Doc references ([`42f021b`](https://github.com/maidsafe/safe_network/commit/42f021b0974a275e1184131cb6621cb0041454e7))
    - Implement storage error ([`e6101a5`](https://github.com/maidsafe/safe_network/commit/e6101a5ef537e1d56722bab86c7fd45c9d964bc9))
    - Move chunk into chunks in storage ([`4223455`](https://github.com/maidsafe/safe_network/commit/422345591d989c846151ccca36d0af8b67aaeccf))
    - Move register into registers in storage ([`b198a36`](https://github.com/maidsafe/safe_network/commit/b198a36220c6a5fe39227c72b5a050dcb351c0cd))
    - Move register into storage mod ([`267399c`](https://github.com/maidsafe/safe_network/commit/267399c6aa597c114706532fddcaf5167dd69441))
    - Move address into storage ([`7201b61`](https://github.com/maidsafe/safe_network/commit/7201b6186a520bc3ca23e07cfc287e8a7197a5af))
    - Remove unnecessary indirection for regstore ([`01f75ac`](https://github.com/maidsafe/safe_network/commit/01f75ac286736ec8df346aa41328604dbb68af38))
    - Remove used space ([`1e63801`](https://github.com/maidsafe/safe_network/commit/1e63801d2e3dcfa3aeb27cb3cbdc6e46468a44cb))
    - Move storage to protocol ([`651c7f5`](https://github.com/maidsafe/safe_network/commit/651c7f53928847cf604bc1b1a9f3eb2df2f081ae))
    - Move log dir param one level up ([`8ebe87e`](https://github.com/maidsafe/safe_network/commit/8ebe87e140fbc7c3db47288f2f5a31ee283e488a))
    - Don't double handle cfg variant ([`5e943fe`](https://github.com/maidsafe/safe_network/commit/5e943fe0c828a56a0f6ba047dbf378af605d43ac))
    - Add fixes from review comments ([`bb66afe`](https://github.com/maidsafe/safe_network/commit/bb66afeaa2151427d39d794bbdb9916c9e116c24))
    - Update readme client cli user instructions ([`0b810c3`](https://github.com/maidsafe/safe_network/commit/0b810c3706c04417e10ec1fd98e12a67b1b686c9))
    - Fix cli files upload and download ([`23b4a04`](https://github.com/maidsafe/safe_network/commit/23b4a0485a744f524666095cb61e8aef63a48fdd))
    - Remove unused dep ([`291a38a`](https://github.com/maidsafe/safe_network/commit/291a38a492ea33c757a12e43b0a10963d9967cd4))
    - Simplify amount parsing for wallet send ([`d537525`](https://github.com/maidsafe/safe_network/commit/d5375254ebd47e223f98bcb90df9b155f914374b))
    - Fix subcmds ([`74d6502`](https://github.com/maidsafe/safe_network/commit/74d6502ebbf76cf3698c253e417db562c6a11e3b))
    - Move subcmd impls to their definition ([`5f22ab8`](https://github.com/maidsafe/safe_network/commit/5f22ab864ac0c7de045c27d75a712e13f5a4723b))
    - Use subcmds ([`826bb0a`](https://github.com/maidsafe/safe_network/commit/826bb0a646a9b69df0f62a4410108c8c9a3b7926))
    - Reduce conflict resolve in rebase ([`624ac90`](https://github.com/maidsafe/safe_network/commit/624ac902974d9727acea10ed1d2a1a5a7895abb9))
    - Make rpc urls https ([`8cd5a96`](https://github.com/maidsafe/safe_network/commit/8cd5a96a0ce4bea00fe760c393518d684d7bbbcc))
    - Use hash of PeerId to calculate xorname instead of chopping bytes ([`39b82e2`](https://github.com/maidsafe/safe_network/commit/39b82e2879b95a6ce7ded6bc7fc0690d2398f27c))
    - Adding example client app for node gRPC service ([`420ee5e`](https://github.com/maidsafe/safe_network/commit/420ee5ef7038ea311bfe6d09fd6adf0c124a1141))
    - Exposing a gRPC interface on safenode bin/app ([`5b266b8`](https://github.com/maidsafe/safe_network/commit/5b266b8bbd1f46d8b87917d0573377ff1ecaf2f7))
    - Error on cli invalid amount ([`728dc69`](https://github.com/maidsafe/safe_network/commit/728dc69c1a4ef75a96552984b6428bbbec226696))
    - Impl simple cli for wallet ops, sending ([`0b365b5`](https://github.com/maidsafe/safe_network/commit/0b365b51bba9cde4a9c50f6884f5081d239eed6d))
    - Client CLI confirming dead node gone in closest ([`3fc4f20`](https://github.com/maidsafe/safe_network/commit/3fc4f20e1e6f7a5efa1aba660aed98297fe02df4))
    - Lower mdns query interval for client stability ([`c3d7e4a`](https://github.com/maidsafe/safe_network/commit/c3d7e4a6780e8d010ca4d9f05908155df77124d2))
    - Move wallet ops to kadclient ([`452c0df`](https://github.com/maidsafe/safe_network/commit/452c0df869b3398673bb61a0c9f19509f39ad044))
    - Move respective ops into fns for wallet ([`3b1ab1b`](https://github.com/maidsafe/safe_network/commit/3b1ab1b7e8e0ce37bee64b462d5f230bf079f65b))
    - Move respective ops into fns ([`35a01e7`](https://github.com/maidsafe/safe_network/commit/35a01e7fd9942964f01746be54587e65444b95d8))
    - Impl simple cli for wallet ops ([`cf4e1c2`](https://github.com/maidsafe/safe_network/commit/cf4e1c2fbf6735641faa86ec6078b2fe686adba7))
    - Dial peers on startup ([`6a45565`](https://github.com/maidsafe/safe_network/commit/6a4556565df6689a0bfe0450fc9ac69d74b23ec0))
    - Log when a peer disconnects ([`4c4b19e`](https://github.com/maidsafe/safe_network/commit/4c4b19e55892ece1bd408a736bd21ea5c6ea3bf1))
    - Move node transfer logic to protocol ([`b61dfa0`](https://github.com/maidsafe/safe_network/commit/b61dfa0a5a2f5051d7613d28760e3a37f176e0f8))
    - Improve naming ([`18f2e86`](https://github.com/maidsafe/safe_network/commit/18f2e869f85fb096d3998e89ea29e54c7c7902d4))
    - Ensure testnet launch fails if build fails ([`1457a45`](https://github.com/maidsafe/safe_network/commit/1457a453341e35ad3fbf426b4e1fa4a57a753761))
    - Register spends in the network ([`edff23e`](https://github.com/maidsafe/safe_network/commit/edff23ed528515ea99361df89ea0f46e99a856e8))
    - Use online transfer in client ([`56672e3`](https://github.com/maidsafe/safe_network/commit/56672e3c7d91053f2c3b37c24dc1cbac54c9e2e4))
    - Fix typo ([`ab5c82e`](https://github.com/maidsafe/safe_network/commit/ab5c82e2fe63b43f4c8c35848cae8edc0dd2b6d2))
    - Doc updates ([`ffe9dfe`](https://github.com/maidsafe/safe_network/commit/ffe9dfe50b7fcec30b5fe6103d033b042b1cb93f))
    - Add online transfer logic ([`4e9c007`](https://github.com/maidsafe/safe_network/commit/4e9c0076f010bf796fbef2891839872bfd382b49))
    - Rearrange the code ([`66bf69a`](https://github.com/maidsafe/safe_network/commit/66bf69a627de5c54f30cb2591f22932b2edc2031))
    - Instantiate wallet in client ([`e579202`](https://github.com/maidsafe/safe_network/commit/e57920279f352d8c02139138e4edc45556228ad4))
    - Use load_from in tests ([`ee46ba1`](https://github.com/maidsafe/safe_network/commit/ee46ba19ab692dbdbab5240c1abea4be24a2093a))
    - Store and load from disk ([`33b533f`](https://github.com/maidsafe/safe_network/commit/33b533f99af1b1e20cea5868636b478df9aed9ec))
    - Clarify the need for NotADoubleSpendAttempt ([`33b6a87`](https://github.com/maidsafe/safe_network/commit/33b6a872a3f15087e78ec9df8b3aa708960a173b))
    - Misc fixes from pr 95 comments ([`9a1a6b6`](https://github.com/maidsafe/safe_network/commit/9a1a6b6d460cd4686044f4ccd65f208c5013e1ff))
    - Extend kadclient to up-/download files ([`16ea0a7`](https://github.com/maidsafe/safe_network/commit/16ea0a77993015cf9f00c4933edca0854e13cc87))
    - Make long error variants simpler ([`714347f`](https://github.com/maidsafe/safe_network/commit/714347f7ceae28a3c1bfcbcf17a96193d28092ae))
    - Clarify docs ([`7876c9d`](https://github.com/maidsafe/safe_network/commit/7876c9d02f4cccf2f3d0f9c23475100927a40ece))
    - Remove unnecessary indirection ([`3c8b583`](https://github.com/maidsafe/safe_network/commit/3c8b58386dd90499ee65097378d5edccab801f3d))
    - Distinguish transfer modules ([`dd845b9`](https://github.com/maidsafe/safe_network/commit/dd845b970c2e475b0aec8081eba28ce6f1bc6015))
    - Additional Register client API ([`72554f3`](https://github.com/maidsafe/safe_network/commit/72554f3f3073189d9c59afb23f98e6cc8c73c811))
    - Add additional layer of race prevention ([`e31e4d3`](https://github.com/maidsafe/safe_network/commit/e31e4d34bf75129514218c9ff4ceeed1b84651c3))
    - Add &mut self to transfers fn signatures ([`00cce80`](https://github.com/maidsafe/safe_network/commit/00cce808950c5eb0a346ecf07b3a9d40dbfc88de))
    - Rename Dbc cmd to SpendDbc ([`bf72aff`](https://github.com/maidsafe/safe_network/commit/bf72aff8e265cb67d0a48e4f5979370e7b77ba15))
    - Select majority of same spends ([`17daccb`](https://github.com/maidsafe/safe_network/commit/17daccbd2b42acd1b9727ffa5b4e2e8f0df9142c))
    - Connect spends, fees and the msgs ([`75ee18f`](https://github.com/maidsafe/safe_network/commit/75ee18f11787d31b0126dcec96142e663f21da8d))
    - Vanishing outputs #92 ([`a41bc93`](https://github.com/maidsafe/safe_network/commit/a41bc935855112bc129d81fdac4f75667088d757))
    - Add the transfer fees and spend queue logic ([`e28caec`](https://github.com/maidsafe/safe_network/commit/e28caece21bf214f3ad5cead91cbfe99476bb8b9))
    - Update and extend docs ([`8039166`](https://github.com/maidsafe/safe_network/commit/8039166f53839cb56d421421b45b618220f19fd1))
    - Use latest sn_dbc ([`c800a27`](https://github.com/maidsafe/safe_network/commit/c800a2758330b91559980d11ad05d48936c5a546))
    - Additional cleanup and organisation ([`b075101`](https://github.com/maidsafe/safe_network/commit/b075101a173211e422544db9f11597a1cd770eab))
    - Improve file org and some cleanup ([`82323fb`](https://github.com/maidsafe/safe_network/commit/82323fbdb1810bcf1e4c70ed54550499231434bf))
    - Make wallet pass sending test ([`c496216`](https://github.com/maidsafe/safe_network/commit/c496216ee15e97a110e30851c42144376676b045))
    - Chore: remove commented out code - This is fee related stuff that will be added in later. ([`4646c89`](https://github.com/maidsafe/safe_network/commit/4646c897ae58735e728f1dc730577d506ffd0ef0))
    - Impl reissue for tests ([`197e056`](https://github.com/maidsafe/safe_network/commit/197e056ed1628be48c6d4e115fbeb1f02d167746))
    - Implement local wallet ([`ae0c077`](https://github.com/maidsafe/safe_network/commit/ae0c077f7af8c63cef28a92ad41478a7bb5fef68))
    - Register client API ([`fd7b176`](https://github.com/maidsafe/safe_network/commit/fd7b176516254630eff28f12a1693fc52a9a74a8))
    - Network CI tests involves client actions ([`6ad9038`](https://github.com/maidsafe/safe_network/commit/6ad903878c797fc49c85f80bcd56278bbebee434))
    - Specify ip and port to listen on ([`4539a12`](https://github.com/maidsafe/safe_network/commit/4539a12004a0321b143d5958bf77b1071e91708d))
    - Random query on peer added ([`a6b9448`](https://github.com/maidsafe/safe_network/commit/a6b9448a113bdbdaa012ffa44689f10939ddfe37))
    - Client should not be present inside closest_peers ([`6040e2d`](https://github.com/maidsafe/safe_network/commit/6040e2d2be6a8198d5cae73f70e7d815262f3352))
    - Validate closest peers ([`24bf659`](https://github.com/maidsafe/safe_network/commit/24bf65976123eba764f5b3193f1e09a92412a135))
    - Avoid lost spawned handler ([`9f34249`](https://github.com/maidsafe/safe_network/commit/9f342492dc702656f961991f9e3e5ec991c94e90))
    - Update due to libp2p new version ([`b19cafc`](https://github.com/maidsafe/safe_network/commit/b19cafca11cf4469e3f235105a3e53bc07f33204))
    - Fix old terminology in comment ([`55e385d`](https://github.com/maidsafe/safe_network/commit/55e385db4d87040b452ac60ef3137ea7ab7e8960))
    - Remove commented out tests ([`3a6c508`](https://github.com/maidsafe/safe_network/commit/3a6c5085048ae1cc1fc79afbfd417a5fea4859b6))
    - Comment updates ([`2c8137c`](https://github.com/maidsafe/safe_network/commit/2c8137ce1445f734b9a2e2ef14bbe8b10c83ee9a))
    - Add file apis and self encryption ([`33082c1`](https://github.com/maidsafe/safe_network/commit/33082c1af4ea92e507db0ab6c1d2ec42d5e8470b))
    - Move double spend same hash check ([`ef4bd4d`](https://github.com/maidsafe/safe_network/commit/ef4bd4d53787e53800e7feef1e0575c58c20e5e1))
    - Remove some paths to simplify the code ([`139c7f3`](https://github.com/maidsafe/safe_network/commit/139c7f37234da8b79429307b6da6eedbac9daae6))
    - Remove unnecessary conversion of hash ([`351ce80`](https://github.com/maidsafe/safe_network/commit/351ce80063367db32778d1384896639cd34b4550))
    - Reference latest version of sn_dbc ([`a1702bc`](https://github.com/maidsafe/safe_network/commit/a1702bca4e4b66249f100b36319dc7f50a1af8fc))
    - Remove invalid spend broadcasts ([`60e2f29`](https://github.com/maidsafe/safe_network/commit/60e2f2961e1fa08d5700039fa362755a68143ebf))
    - Validate parents and doublespends ([`fc95249`](https://github.com/maidsafe/safe_network/commit/fc9524992474abee593c1be203e640cbcb0c9be9))
    - Merge pull request #77 from grumbach/cleanup ([`0745a29`](https://github.com/maidsafe/safe_network/commit/0745a29863cd1b6de8798089936e62d834fc5798))
    - Remove empty file ([`08db243`](https://github.com/maidsafe/safe_network/commit/08db243d8db1e5891cc97c2403324cc77e3d049c))
    - Count self in the close group ([`179072e`](https://github.com/maidsafe/safe_network/commit/179072ec7c66fe6689b77d47ef6bf211254054b6))
    - Replace generic Error types with more specific ones ([`08e2479`](https://github.com/maidsafe/safe_network/commit/08e2479d752f23c0343219c88287d6ae4c550473))
    - Correct termination of get_closest_peers ([`ac488db`](https://github.com/maidsafe/safe_network/commit/ac488dbcafcf5f999f990eaf156bedf15213570c))
    - Implement Client API to use a Kad swarm in client-only mode ([`6ef0ef9`](https://github.com/maidsafe/safe_network/commit/6ef0ef9c7375bb6d690bd464269a1f6c38e188af))
    - Use close group var ([`6cc8450`](https://github.com/maidsafe/safe_network/commit/6cc84506304c895cda63d7588d9b938aa8aa6039))
    - Boundary of get_closest_peers ([`2e78160`](https://github.com/maidsafe/safe_network/commit/2e781600e52321092ce5a903a9f9106e5374d17d))
    - Update to released sn_dbc ([`2161cf2`](https://github.com/maidsafe/safe_network/commit/2161cf223c9cdfe055b11bf2a436b36077392782))
    - Feat(spends): match on spend errors - This will allow broadcasting an invalid spend (wether parent or current spend) to respective close group (TBD). ([`600bd37`](https://github.com/maidsafe/safe_network/commit/600bd37945f788f818430bf3e00830e1488bc5ed))
    - Feat(dbcs): validate input parents - This verifies that the spend parents are valid, which is a requisite for storing this spend. - After this spend has been stored, it is up to the client to query all close nodes and verify that it is recognised by enough nodes. That then makes the spend valid. - NB: More validations might be needed. ([`1cc8ff9`](https://github.com/maidsafe/safe_network/commit/1cc8ff981c34028d0a4060db81d4e8353bb0706e))
    - Integrate to the system ([`145ec30`](https://github.com/maidsafe/safe_network/commit/145ec301fff026ab46f57c62e3093403f0055963))
    - Refactor(node): don't have client fn on nodes - This implements a Client and removes the client-specific logic from Node. ([`db9ee40`](https://github.com/maidsafe/safe_network/commit/db9ee4007447c449a89f4b8956e6e207f9c288dd))
    - Various minor adjustments ([`7fd46f8`](https://github.com/maidsafe/safe_network/commit/7fd46f8f391be0ef315d0876f3d569c806aa3b70))
    - Fix naming ([`9b52e33`](https://github.com/maidsafe/safe_network/commit/9b52e333699454179f298a44d2efd1c62bf49123))
    - Use tokio everywhere ([`5cd9f4a`](https://github.com/maidsafe/safe_network/commit/5cd9f4af674a1e19ea64b1092959477afdeb4040))
    - Use the closest nodes to put/get data ([`2c3657d`](https://github.com/maidsafe/safe_network/commit/2c3657d58884acd239d82e3099052a970fad8493))
    - Disable random restart ([`29f726a`](https://github.com/maidsafe/safe_network/commit/29f726ad86c111f3ac7f4fa858fe7f5ba6b2996d))
    - Remove chunk specific api ([`ac754fd`](https://github.com/maidsafe/safe_network/commit/ac754fdf24919065cc1292f4df7e6dab31388fcd))
    - Flatten errors ([`9bbee06`](https://github.com/maidsafe/safe_network/commit/9bbee062afe133dea986350ae8480b63bdce131f))
    - Implement an in-memory Register storage ([`186f493`](https://github.com/maidsafe/safe_network/commit/186f49374e1897d7ddfc05499783d717a89704cd))
    - Implement an in-memory Chunk storage ([`e6bb10e`](https://github.com/maidsafe/safe_network/commit/e6bb10ea9d5e829826520384fbfc3a6c61f7c494))
    - Remove deps, remove EnvFilter ([`de04d62`](https://github.com/maidsafe/safe_network/commit/de04d62f6dc155616c14e0f4a07f3b8205398b1b))
    - Use tokio executor all over ([`0e9bc3d`](https://github.com/maidsafe/safe_network/commit/0e9bc3da11878ac9357eb76c8cf61fd2a83a8735))
    - Chore: some further request division - Also aligns some fn and variable names. ([`51b51a7`](https://github.com/maidsafe/safe_network/commit/51b51a72a0a50a0921ba83145d1b61ad25a6143f))
    - Add a basic level of churn to nodes ([`7543586`](https://github.com/maidsafe/safe_network/commit/7543586c0ad461c54bce95458660d6e2b7ee9492))
    - Fix naming ([`d748fcd`](https://github.com/maidsafe/safe_network/commit/d748fcd6e6c3ba604fb898b3be8b73e96270e993))
    - Add docs + clippy fixes ([`ba7c741`](https://github.com/maidsafe/safe_network/commit/ba7c74175e7082f6a2d4afc64a85be2c56b9d8c9))
    - Make req/resp generic ([`5ce1e89`](https://github.com/maidsafe/safe_network/commit/5ce1e89c56cebd9c61f8032c2ca86c258e5f033a))
    - Add env filter and strip back testnet bin ([`892c8b3`](https://github.com/maidsafe/safe_network/commit/892c8b3abf332fbbe100bf04c0b04cc9e67be828))
    - Include reference impl ([`3374b3b`](https://github.com/maidsafe/safe_network/commit/3374b3b6bcd2e010ef31ec46c5bb87515d8ba6f7))
    - Use Error enum ([`500566d`](https://github.com/maidsafe/safe_network/commit/500566d66c08aa89ccd2a0ad43ef99b5d83ce5c3))
    - Implement req/resp to store and retrieve chunks ([`a77b33d`](https://github.com/maidsafe/safe_network/commit/a77b33df2a846423eabf8debfcf15f0ac50f085d))
    - Use libp2p-quic instead of the quic feature ([`c6ae34f`](https://github.com/maidsafe/safe_network/commit/c6ae34f3a8abb5657e08b234e9f1810ee1435ec1))
    - Clippy lints ([`5e63386`](https://github.com/maidsafe/safe_network/commit/5e633868773e42c13326c2f52790c94d4cd88ae0))
    - Enable log level through env variable ([`63081bc`](https://github.com/maidsafe/safe_network/commit/63081bc27b6f6d3280ad3e55dddf934177368569))
    - Use quic transport protocol ([`9980d85`](https://github.com/maidsafe/safe_network/commit/9980d85708e566a31b4f0da359c62202237ab924))
    - Search for xorname ([`7571c17`](https://github.com/maidsafe/safe_network/commit/7571c17df10fb5259dd1ca7d41a8ef9a7857225d))
    - 25 nodes and a couple of searches ([`1a22722`](https://github.com/maidsafe/safe_network/commit/1a22722198b5aecaca00dc167c7084d06f39160b))
    - Init of search ([`13ac616`](https://github.com/maidsafe/safe_network/commit/13ac6161460a4194d52065d5cc3b2a0f21d36906))
    - Receive on cmd channel ([`4c6cada`](https://github.com/maidsafe/safe_network/commit/4c6cadacf3e7b20faabfb4434fdbc74c43c5edb2))
    - Refactor out swarm and provide channel ([`55ca268`](https://github.com/maidsafe/safe_network/commit/55ca268a5fe5f90f5f67a37a626fe46ccbe638c8))
    - Kadnode attempt w/ tcp ([`f063f84`](https://github.com/maidsafe/safe_network/commit/f063f8442608f074dbaf5c4b15dcb419db145fcf))
    - Update safenode/src/stableset/mod.rs ([`e258f6f`](https://github.com/maidsafe/safe_network/commit/e258f6fb0bf9a14fe2ac515f54fab76ffee64f8f))
    - Make response stream optional again, respond to sender over stream if existing ([`b827c20`](https://github.com/maidsafe/safe_network/commit/b827c2028f59191a7f84a58f23c9d5dfb3bd7b11))
    - Refactor out stable set update from msg processing ([`0bcce42`](https://github.com/maidsafe/safe_network/commit/0bcce425ef56b54095103c5a8cfb3787b8a94696))
    - Refactor out stable set msg received event extraction ([`af56c5e`](https://github.com/maidsafe/safe_network/commit/af56c5ec20c84516e2330b9d4077dc30c696df4e))
    - Merge pull request #19 from joshuef/ProperlyhandleJoins ([`8f54f27`](https://github.com/maidsafe/safe_network/commit/8f54f27ea0d2237891bb13aa44025e0e6d13be65))
    - Properly handle joined nodes before sync ([`bbe5dce`](https://github.com/maidsafe/safe_network/commit/bbe5dce01ab88e33caf9106338506ec98aa48387))
    - Unify membership and stable_set ([`48e0465`](https://github.com/maidsafe/safe_network/commit/48e04652f5ddd66f43f87455b4cf884c23bc96e6))
    - Update gitignore to remove trunk ([`9bbadd6`](https://github.com/maidsafe/safe_network/commit/9bbadd672ebb1aa4bb66a538b921f5c3691fe12a))
    - Share->witness & break up some methods ([`69bc68d`](https://github.com/maidsafe/safe_network/commit/69bc68dad31ef2169398bf3a00c77422f8c33334))
    - Some joining, but not enough sync ([`bd396cf`](https://github.com/maidsafe/safe_network/commit/bd396cf46e5d1a55dc74cc18412e5b8816df05b5))
    - Accept sync msg, update valid comm targets ([`02e3ee8`](https://github.com/maidsafe/safe_network/commit/02e3ee80fde50d909984e5b80b6b0300d42367bb))
    - Send sync msg after handling ([`8c34f90`](https://github.com/maidsafe/safe_network/commit/8c34f90a7ad3c3670b415b9845aac46488a50965))
    - Start sending joins ([`1b92b34`](https://github.com/maidsafe/safe_network/commit/1b92b346f07aee6b92f782a66257b148dcb45785))
    - Merge pull request #8 from joshuef/RandomPortNodes ([`34b2bfb`](https://github.com/maidsafe/safe_network/commit/34b2bfb7746fcd16f08aa2431181a502135b2865))
    - Initial comms by writing 127.0.0.1 ip addre for genesis ([`6190d22`](https://github.com/maidsafe/safe_network/commit/6190d222e04904baad12070f3893c2d0c425238a))
    - Add some logging to dirs per node ([`514e815`](https://github.com/maidsafe/safe_network/commit/514e8153bfc33cd5bb12e7998dd065e5f5c30c4c))
    - Cargo fix ([`f772949`](https://github.com/maidsafe/safe_network/commit/f772949320519c868a5e2ffc3b611aa138567afd))
    - Use a random port @ startup, write config if none exists ([`e7f1da1`](https://github.com/maidsafe/safe_network/commit/e7f1da121e9b7afd2784caeab1fd8b826c47fa85))
    - Merge pull request #7 from b-zee/refactor-set-socket-address-by-argument ([`2f58e08`](https://github.com/maidsafe/safe_network/commit/2f58e088edeb8b28077c637ed5d53efdf9535432))
    - Rename get_config ([`e17a189`](https://github.com/maidsafe/safe_network/commit/e17a1890d3254abc5e258cf662bfd79e71080949))
    - Set socket addr by argument ([`c5831ac`](https://github.com/maidsafe/safe_network/commit/c5831ace461627781066ff2f8a75feda524f2ca7))
    - Merge pull request #6 from joshuef/AddTestnetBin ([`874c014`](https://github.com/maidsafe/safe_network/commit/874c01401acf980a226839247514e4bd69a58273))
    - Convert safenode to bin ([`e40ac52`](https://github.com/maidsafe/safe_network/commit/e40ac52e83be846c2c026d9618431e0269a8116b))
    - Create a basic workspace for the repo ([`0074ea6`](https://github.com/maidsafe/safe_network/commit/0074ea6ce8f4689c9a6bc42e94539fd42e564a7a))
    - Initial copy of testnet bin with basic tweaks. ([`fa4b3ea`](https://github.com/maidsafe/safe_network/commit/fa4b3eacb4930749ad229cf2dbd26949b0a77a7e))
    - Convert safenode to bin ([`6a318fa`](https://github.com/maidsafe/safe_network/commit/6a318fa7af40360c2ea8b83f670ce3f51b0904bc))
    - Create a basic workspace for the repo ([`368f3bc`](https://github.com/maidsafe/safe_network/commit/368f3bcdd1864c41c63904233b260b8d2df0a15a))
</details>

