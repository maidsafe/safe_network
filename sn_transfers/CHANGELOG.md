# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.18.6](https://github.com/joshuef/safe_network/compare/sn_transfers-v0.18.5...sn_transfers-v0.18.6) - 2024-06-04

### Other
- release
- release

## [0.18.5](https://github.com/joshuef/safe_network/compare/sn_transfers-v0.18.4...sn_transfers-v0.18.5) - 2024-06-04

### Fixed
- *(transfer)* mismatched key shall result in decryption error

### Other
- *(transfer)* make discord_name decryption backward compatible

## [0.18.4](https://github.com/joshuef/safe_network/compare/sn_transfers-v0.18.3...sn_transfers-v0.18.4) - 2024-06-03

### Fixed
- enable compile time sk setting for faucet/genesis

## [0.18.2](https://github.com/joshuef/safe_network/compare/sn_transfers-v0.18.1...sn_transfers-v0.18.2) - 2024-06-03

### Added
- *(faucet)* write foundation cash note to disk
- *(keys)* enable compile or runtime override of keys

### Other
- use secrets during build process

## [0.18.1](https://github.com/joshuef/safe_network/compare/sn_transfers-v0.18.0...sn_transfers-v0.18.1) - 2024-05-24

### Added
- use default keys for genesis, or override
- use different key for payment forward
- remove two uneeded env vars
- pass genesis_cn pub fields separate to hide sk
- hide genesis keypair
- hide genesis keypair
- pass sk_str via cli opt
- *(node)* use separate keys of Foundation and Royalty
- *(wallet)* ensure genesis wallet attempts to load from local on init first
- *(faucet)* make gifting server feat dependent
- tracking beta rewards from the DAG
- *(audit)* collect payment forward statistics
- *(node)* periodically forward reward to specific address
- spend reason enum and sized cipher

### Fixed
- correct genesis_pk naming
- genesis_cn public fields generated from hard coded value
- invalid spend reason in data payments

### Other
- *(transfers)* comment and naming updates for clarity
- log genesis PK
- rename improperly named foundation_key
- reconfigure local network owner args
- *(refactor)* stabilise node size to 4k records,
- use const for default user or owner
- resolve errors after reverts
- Revert "feat(node): make spend and cash_note reason field configurable"
- Revert "feat: spend shows the purposes of outputs created for"
- Revert "chore: rename output reason to purpose for clarity"
- Revert "feat(cli): track spend creation reasons during audit"
- Revert "chore: refactor CASH_NOTE_REASON strings to consts"
- Revert "chore: address review comments"
- *(node)* use proper SpendReason enum
- add consts

## [0.18.0-alpha.1](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.18.0-alpha.0...sn_transfers-v0.18.0-alpha.1) - 2024-05-07

### Added
- *(cli)* track spend creation reasons during audit
- spend shows the purposes of outputs created for
- *(node)* make spend and cash_note reason field configurable
- *(cli)* generate a mnemonic as wallet basis if no wallet found
- *(transfers)* do not genereate wallet by default
- [**breaking**] renamings in CashNote
- [**breaking**] rename token to amount in Spend
- unit testing dag, double spend poisoning tweaks

### Fixed
- create faucet via account load or generation
- transfer tests for HotWallet creation
- *(client)* move acct_packet mnemonic into client layer
- typo

### Other
- *(versions)* sync versions with latest crates.io vs
- address review comments
- refactor CASH_NOTE_REASON strings to consts
- rename output reason to purpose for clarity
- addres review comments
- *(transfers)* reduce error size
- *(deps)* bump dependencies
- *(transfer)* unit tests for PaymentQuote
- *(release)* sn_auditor-v0.1.7/sn_client-v0.105.3/sn_networking-v0.14.4/sn_protocol-v0.16.3/sn_build_info-v0.1.7/sn_transfers-v0.17.2/sn_peers_acquisition-v0.2.10/sn_cli-v0.90.4/sn_faucet-v0.4.9/sn_metrics-v0.1.4/sn_node-v0.105.6/sn_service_management-v0.2.4/sn-node-manager-v0.7.4/sn_node_rpc_client-v0.6.8/token_supplies-v0.1.47
- *(release)* sn_auditor-v0.1.3-alpha.0/sn_client-v0.105.3-alpha.0/sn_networking-v0.14.2-alpha.0/sn_protocol-v0.16.2-alpha.0/sn_build_info-v0.1.7-alpha.0/sn_transfers-v0.17.2-alpha.0/sn_peers_acquisition-v0.2.9-alpha.0/sn_cli-v0.90.3-alpha.0/sn_node-v0.105.4-alpha.0/sn-node-manager-v0.7.3-alpha.0/sn_faucet-v0.4.4-alpha.0/sn_service_management-v0.2.2-alpha.0/sn_node_rpc_client-v0.6.4-alpha.0

## [0.17.1](https://github.com/joshuef/safe_network/compare/sn_transfers-v0.17.0...sn_transfers-v0.17.1) - 2024-03-28

### Added
- *(transfers)* implement WalletApi to expose common methods

### Fixed
- *(uploader)* clarify the use of root and wallet dirs

## [0.17.0](https://github.com/joshuef/safe_network/compare/sn_transfers-v0.16.5...sn_transfers-v0.17.0) - 2024-03-27

### Added
- *(faucet)* rate limit based upon wallet locks
- *(transfers)* enable client to check if a quote has expired
- *(transfers)* [**breaking**] support multiple payments for the same xorname
- use Arc inside Client, Network to reduce clone cost

### Other
- *(node)* refactor pricing metrics

## [0.16.5](https://github.com/joshuef/safe_network/compare/sn_transfers-v0.16.4...sn_transfers-v0.16.5) - 2024-03-21

### Added
- refactor DAG, improve error management and security
- dag error recording

## [0.16.4](https://github.com/joshuef/safe_network/compare/sn_transfers-v0.16.3...sn_transfers-v0.16.4) - 2024-03-14

### Added
- refactor spend validation

### Other
- improve code quality

## [0.16.3-alpha.1](https://github.com/joshuef/safe_network/compare/sn_transfers-v0.16.3-alpha.0...sn_transfers-v0.16.3-alpha.1) - 2024-03-08

### Added
- [**breaking**] pretty serialisation for unique keys

## [0.16.2](https://github.com/joshuef/safe_network/compare/sn_transfers-v0.16.1...sn_transfers-v0.16.2) - 2024-03-06

### Other
- clean swarm commands errs and spend errors

## [0.16.1](https://github.com/joshuef/safe_network/compare/sn_transfers-v0.16.0...sn_transfers-v0.16.1) - 2024-03-05

### Added
- provide `faucet add` command

## [0.16.0](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.15.9...sn_transfers-v0.16.0) - 2024-02-23

### Added
- use the old serialisation as default, add some docs
- warn about old format when detected
- implement backwards compatible deserialisation
- [**breaking**] custom serde for unique keys

## [0.15.8](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.15.7...sn_transfers-v0.15.8) - 2024-02-20

### Added
- spend and DAG utilities

## [0.15.7](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.15.6...sn_transfers-v0.15.7) - 2024-02-20

### Added
- *(folders)* move folders/files metadata out of Folders entries

## [0.15.6](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.15.5...sn_transfers-v0.15.6) - 2024-02-15

### Added
- *(client)* keep payee as part of storage payment cache

### Other
- minor doc change based on peer review

## [0.15.5](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.15.4...sn_transfers-v0.15.5) - 2024-02-14

### Other
- *(refactor)* move mod.rs files the modern way

## [0.15.4](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.15.3...sn_transfers-v0.15.4) - 2024-02-13

### Fixed
- manage the genesis spend case

## [0.15.3](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.15.2...sn_transfers-v0.15.3) - 2024-02-08

### Other
- copyright update to current year

## [0.15.2](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.15.1...sn_transfers-v0.15.2) - 2024-02-07

### Added
- extendable local state DAG in cli

## [0.15.1](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.15.0...sn_transfers-v0.15.1) - 2024-02-06

### Fixed
- *(node)* derive reward_key from main keypair

## [0.15.0](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.43...sn_transfers-v0.15.0) - 2024-02-02

### Other
- *(cli)* minor changes to cli comments
- [**breaking**] renaming LocalWallet to HotWallet as it holds the secret key for signing tx
- *(readme)* add instructions of out-of-band transaction signing

## [0.14.43](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.42...sn_transfers-v0.14.43) - 2024-01-29

### Other
- *(sn_transfers)* making some functions/helpers to be constructor methods of public structs

## [0.14.42](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.41...sn_transfers-v0.14.42) - 2024-01-25

### Added
- client webtransport-websys feat

## [0.14.41](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.40...sn_transfers-v0.14.41) - 2024-01-24

### Fixed
- dont lock files with wasm

### Other
- make tokio dev dep for transfers

## [0.14.40](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.39...sn_transfers-v0.14.40) - 2024-01-22

### Added
- spend dag utils

## [0.14.39](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.38...sn_transfers-v0.14.39) - 2024-01-18

### Added
- *(faucet)* download snapshot of maid balances

## [0.14.38](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.37...sn_transfers-v0.14.38) - 2024-01-16

### Fixed
- *(wallet)* remove unconfirmed_spends file from disk when all confirmed

## [0.14.37](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.36...sn_transfers-v0.14.37) - 2024-01-15

### Fixed
- *(client)* do not store paying-out cash_notes into disk
- *(client)* cache payments via disk instead of memory map

### Other
- *(client)* collect wallet handling time statistics

## [0.14.36](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.35...sn_transfers-v0.14.36) - 2024-01-10

### Added
- *(transfers)* exposing APIs to build and send cashnotes from transactions signed offline
- *(transfers)* include the derivation index of inputs for generated unsigned transactions
- *(transfers)* exposing an API to create unsigned transfers to be signed offline later on

### Other
- fixup send_spends and use ExcessiveNanoValue error
- *(transfers)* solving clippy issues about complex fn args

## [0.14.35](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.34...sn_transfers-v0.14.35) - 2024-01-09

### Added
- *(client)* extra sleep between chunk verification

## [0.14.34](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.33...sn_transfers-v0.14.34) - 2024-01-09

### Added
- *(cli)* safe wallet create saves new key

## [0.14.33](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.32...sn_transfers-v0.14.33) - 2024-01-08

### Other
- more doc updates to readme files

## [0.14.32](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.31...sn_transfers-v0.14.32) - 2024-01-05

### Other
- add clippy unwrap lint to workspace

## [0.14.31](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.30...sn_transfers-v0.14.31) - 2023-12-19

### Added
- network royalties through audit POC

## [0.14.30](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.29...sn_transfers-v0.14.30) - 2023-12-18

### Added
- *(transfers)* spent keys and created for others removed
- *(transfers)* add api for cleaning up CashNotes

## [0.14.29](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.28...sn_transfers-v0.14.29) - 2023-12-14

### Other
- *(protocol)* print the first six hex characters for every address type

## [0.14.28](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.27...sn_transfers-v0.14.28) - 2023-12-12

### Added
- *(transfers)* make wallet read resiliant to concurrent writes

## [0.14.27](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.26...sn_transfers-v0.14.27) - 2023-12-06

### Added
- *(wallet)* basic impl of a watch-only wallet API

### Other
- *(wallet)* adding unit tests for watch-only wallet impl.
- *(wallet)* another refactoring removing more redundant and unused wallet code
- *(wallet)* major refactoring removing redundant and unused code

## [0.14.26](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.25...sn_transfers-v0.14.26) - 2023-12-06

### Other
- remove some needless cloning
- remove needless pass by value
- use inline format args
- add boilerplate for workspace lints

## [0.14.25](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.24...sn_transfers-v0.14.25) - 2023-12-05

### Fixed
- protect against amounts tampering and incomplete spends attack

## [0.14.24](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.23...sn_transfers-v0.14.24) - 2023-12-05

### Other
- *(transfers)* tidier debug methods for Transactions

## [0.14.23](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.22...sn_transfers-v0.14.23) - 2023-11-29

### Added
- verify all the way to genesis
- verify spends through the cli

### Fixed
- genesis check security flaw

## [0.14.22](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.21...sn_transfers-v0.14.22) - 2023-11-28

### Added
- *(transfers)* serialise wallets and transfers data with MsgPack instead of bincode

## [0.14.21](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.20...sn_transfers-v0.14.21) - 2023-11-23

### Added
- move derivation index random method to itself

## [0.14.20](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.19...sn_transfers-v0.14.20) - 2023-11-22

### Other
- optimise log format of DerivationIndex

## [0.14.19](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.18...sn_transfers-v0.14.19) - 2023-11-20

### Added
- *(networking)* shortcircuit response sending for replication

## [0.14.18](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.17...sn_transfers-v0.14.18) - 2023-11-20

### Added
- quotes

### Fixed
- use actual quote instead of dummy

## [0.14.17](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.16...sn_transfers-v0.14.17) - 2023-11-16

### Added
- massive cleaning to prepare for quotes

### Fixed
- wrong royaltie amount
- cashnote mixup when 2 of them are for the same node

## [0.14.16](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.15...sn_transfers-v0.14.16) - 2023-11-15

### Added
- *(royalties)* make royalties payment to be 15% of the total storage cost

## [0.14.15](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.14...sn_transfers-v0.14.15) - 2023-11-14

### Other
- *(royalties)* verify royalties fees amounts

## [0.14.14](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.13...sn_transfers-v0.14.14) - 2023-11-10

### Added
- *(cli)* attempt to reload wallet from disk if storing it fails when receiving transfers online
- *(cli)* new cmd to listen to royalties payments and deposit them into a local wallet

## [0.14.13](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.12...sn_transfers-v0.14.13) - 2023-11-10

### Other
- *(transfers)* more logs around payments...

## [0.14.12](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.11...sn_transfers-v0.14.12) - 2023-11-09

### Other
- simplify when construct payess for storage

## [0.14.11](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.10...sn_transfers-v0.14.11) - 2023-11-02

### Added
- keep transfers in mem instead of heavy cashnotes

## [0.14.10](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.9...sn_transfers-v0.14.10) - 2023-11-01

### Other
- *(node)* don't log the transfers events

## [0.14.9](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.8...sn_transfers-v0.14.9) - 2023-10-30

### Added
- `bincode::serialize` into `Bytes` without intermediate allocation

## [0.14.8](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.7...sn_transfers-v0.14.8) - 2023-10-27

### Added
- *(rpc_client)* show total accumulated balance when decrypting transfers received

## [0.14.7](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.6...sn_transfers-v0.14.7) - 2023-10-26

### Fixed
- typos

## [0.14.6](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.5...sn_transfers-v0.14.6) - 2023-10-24

### Fixed
- *(tests)* nodes rewards tests to account for repayments amounts

## [0.14.5](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.4...sn_transfers-v0.14.5) - 2023-10-24

### Added
- *(payments)* adding unencrypted CashNotes for network royalties and verifying correct payment
- *(payments)* network royalties payment made when storing content

### Other
- *(api)* wallet APIs to account for network royalties fees when returning total cost paid for storage

## [0.14.4](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.3...sn_transfers-v0.14.4) - 2023-10-24

### Fixed
- *(networking)* only validate _our_ transfers at nodes

## [0.14.3](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.2...sn_transfers-v0.14.3) - 2023-10-18

### Other
- Revert "feat: keep transfers in mem instead of mem and i/o heavy cashnotes"

## [0.14.2](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.1...sn_transfers-v0.14.2) - 2023-10-18

### Added
- keep transfers in mem instead of mem and i/o heavy cashnotes

## [0.14.1](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.14.0...sn_transfers-v0.14.1) - 2023-10-17

### Fixed
- *(transfers)* dont overwrite existing payment transactions when we top up

### Other
- adding comments and cleanup around quorum / payment fixes

## [0.14.0](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.13.12...sn_transfers-v0.14.0) - 2023-10-12

### Added
- *(sn_transfers)* dont load Cns from disk, store value along w/ pubkey in wallet
- include protection for deposits

### Fixed
- remove uneeded hideous key Clone trait
- deadlock
- place lock on another file to prevent windows lock issue
- lock wallet file instead of dir
- wallet concurrent access bugs

### Other
- more detailed logging when client creating store cash_note

## [0.13.12](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.13.11...sn_transfers-v0.13.12) - 2023-10-11

### Fixed
- expose RecordMismatch errors and cleanup wallet if we hit that

### Other
- *(transfers)* add somre more clarity around DoubleSpendAttemptedForCashNotes
- *(docs)* cleanup comments and docs
- *(transfers)* remove pointless api

## [0.13.11](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.13.10...sn_transfers-v0.13.11) - 2023-10-10

### Added
- *(transfer)* special event for transfer notifs over gossipsub

## [0.13.10](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.13.9...sn_transfers-v0.13.10) - 2023-10-10

### Other
- *(sn_transfers)* improve transaction build mem perf

## [0.13.9](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.13.8...sn_transfers-v0.13.9) - 2023-10-06

### Added
- feat!(sn_transfers): unify store api for wallet

### Fixed
- readd api to load cash_notes from disk, update tests

### Other
- update comments around RecordNotFound
- remove deposit vs received cashnote disctinction

## [0.13.8](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.13.7...sn_transfers-v0.13.8) - 2023-10-06

### Other
- fix new clippy errors

## [0.13.7](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.13.6...sn_transfers-v0.13.7) - 2023-10-05

### Added
- *(metrics)* enable node monitoring through dockerized grafana instance

## [0.13.6](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.13.5...sn_transfers-v0.13.6) - 2023-10-05

### Fixed
- *(client)* remove concurrency limitations

## [0.13.5](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.13.4...sn_transfers-v0.13.5) - 2023-10-05

### Fixed
- *(sn_transfers)* be sure we store CashNotes before writing the wallet file

## [0.13.4](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.13.3...sn_transfers-v0.13.4) - 2023-10-05

### Added
- use progress bars on `files upload`

## [0.13.3](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.13.2...sn_transfers-v0.13.3) - 2023-10-04

### Added
- *(sn_transfers)* impl From for NanoTokens

### Fixed
- *(sn_transfers)* reuse payment overflow fix

### Other
- *(sn_transfers)* clippy and fmt
- *(sn_transfers)* add reuse cashnote cases
- separate method and write test

## [0.13.2](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.13.1...sn_transfers-v0.13.2) - 2023-10-02

### Added
- remove unused fee output

## [0.13.1](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.13.0...sn_transfers-v0.13.1) - 2023-09-28

### Added
- client to client transfers

## [0.13.0](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.12.2...sn_transfers-v0.13.0) - 2023-09-27

### Added
- deep clean sn_transfers, reduce exposition, remove dead code

### Fixed
- benches
- uncomment benches in Cargo.toml

### Other
- optimise bench
- improve cloning
- udeps

## [0.12.2](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.12.1...sn_transfers-v0.12.2) - 2023-09-25

### Other
- *(transfers)* unused variable removal

## [0.12.1](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.12.0...sn_transfers-v0.12.1) - 2023-09-25

### Other
- udeps
- cleanup renamings in sn_transfers
- remove mostly outdated mocks

## [0.12.0](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.11.15...sn_transfers-v0.12.0) - 2023-09-21

### Added
- rename utxo by CashNoteRedemption
- dusking DBCs

### Fixed
- udeps
- incompatible hardcoded value, add logs

### Other
- remove dbc dust comments
- rename Nano NanoTokens
- improve naming

## [0.11.15](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.11.14...sn_transfers-v0.11.15) - 2023-09-20

### Other
- major dep updates

## [0.11.14](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.11.13...sn_transfers-v0.11.14) - 2023-09-18

### Added
- serialisation for transfers for out of band sending
- generic transfer receipt

### Other
- add more docs
- add some docs

## [0.11.13](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.11.12...sn_transfers-v0.11.13) - 2023-09-15

### Other
- refine log levels

## [0.11.12](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.11.11...sn_transfers-v0.11.12) - 2023-09-14

### Other
- updated the following local packages: sn_protocol

## [0.11.11](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.11.10...sn_transfers-v0.11.11) - 2023-09-13

### Added
- *(register)* paying nodes for Register storage

## [0.11.10](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.11.9...sn_transfers-v0.11.10) - 2023-09-12

### Added
- add tx and parent spends verification
- chunk payments using UTXOs instead of DBCs

### Other
- use updated sn_dbc

## [0.11.9](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.11.8...sn_transfers-v0.11.9) - 2023-09-11

### Other
- *(release)* sn_cli-v0.81.29/sn_client-v0.88.16/sn_registers-v0.2.6/sn_node-v0.89.29/sn_testnet-v0.2.120/sn_protocol-v0.6.6

## [0.11.8](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.11.7...sn_transfers-v0.11.8) - 2023-09-08

### Added
- *(client)* repay for chunks if they cannot be validated

## [0.11.7](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.11.6...sn_transfers-v0.11.7) - 2023-09-05

### Other
- *(release)* sn_cli-v0.81.21/sn_client-v0.88.11/sn_registers-v0.2.5/sn_node-v0.89.21/sn_testnet-v0.2.112/sn_protocol-v0.6.5

## [0.11.6](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.11.5...sn_transfers-v0.11.6) - 2023-09-04

### Other
- updated the following local packages: sn_protocol

## [0.11.5](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.11.4...sn_transfers-v0.11.5) - 2023-09-04

### Other
- updated the following local packages: sn_protocol

## [0.11.4](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.11.3...sn_transfers-v0.11.4) - 2023-09-01

### Other
- *(transfers)* batch dbc storage
- *(transfers)* store dbcs by ref to avoid more clones
- *(transfers)* dont pass by value, this is a clone!
- *(client)* make unconfonfirmed txs btreeset, remove unnecessary cloning
- *(transfers)* improve update_local_wallet

## [0.11.3](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.11.2...sn_transfers-v0.11.3) - 2023-08-31

### Other
- remove unused async

## [0.11.2](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.11.1...sn_transfers-v0.11.2) - 2023-08-31

### Added
- *(node)* node to store rewards in a local wallet

### Fixed
- *(cli)* don't try to create wallet paths when checking balance

## [0.11.1](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.11.0...sn_transfers-v0.11.1) - 2023-08-31

### Other
- updated the following local packages: sn_protocol

## [0.11.0](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.28...sn_transfers-v0.11.0) - 2023-08-30

### Added
- one transfer per data set, mapped dbcs to content addrs
- [**breaking**] pay each chunk holder direct
- feat!(protocol): gets keys with GetStoreCost
- feat!(protocol): get price and pay for each chunk individually
- feat!(protocol): remove chunk merkletree to simplify payment

### Fixed
- *(tokio)* remove tokio fs

### Other
- *(deps)* bump tokio to 1.32.0
- *(client)* refactor client wallet to reduce dbc clones
- *(client)* pass around content payments map mut ref
- *(client)* error out early for invalid transfers

## [0.10.28](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.27...sn_transfers-v0.10.28) - 2023-08-24

### Other
- rust 1.72.0 fixes

## [0.10.27](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.26...sn_transfers-v0.10.27) - 2023-08-18

### Other
- updated the following local packages: sn_protocol

## [0.10.26](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.25...sn_transfers-v0.10.26) - 2023-08-11

### Added
- *(transfers)* add resend loop for unconfirmed txs

## [0.10.25](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.24...sn_transfers-v0.10.25) - 2023-08-10

### Other
- updated the following local packages: sn_protocol

## [0.10.24](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.23...sn_transfers-v0.10.24) - 2023-08-08

### Added
- *(transfers)* add get largest dbc for spending

### Fixed
- *(node)* prevent panic in storage calcs

### Other
- *(faucet)* provide more money
- tidy store cost code

## [0.10.23](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.22...sn_transfers-v0.10.23) - 2023-08-07

### Other
- rename network addresses confusing name method to xorname

## [0.10.22](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.21...sn_transfers-v0.10.22) - 2023-08-01

### Other
- *(networking)* use TOTAL_SUPPLY from sn_transfers

## [0.10.21](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.20...sn_transfers-v0.10.21) - 2023-08-01

### Other
- updated the following local packages: sn_protocol

## [0.10.20](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.19...sn_transfers-v0.10.20) - 2023-08-01

### Other
- *(release)* sn_cli-v0.80.17/sn_client-v0.87.0/sn_registers-v0.2.0/sn_node-v0.88.6/sn_testnet-v0.2.44/sn_protocol-v0.4.2

## [0.10.19](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.18...sn_transfers-v0.10.19) - 2023-07-31

### Fixed
- *(test)* using proper wallets during data_with_churn test

## [0.10.18](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.17...sn_transfers-v0.10.18) - 2023-07-28

### Other
- updated the following local packages: sn_protocol

## [0.10.17](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.16...sn_transfers-v0.10.17) - 2023-07-26

### Other
- updated the following local packages: sn_protocol

## [0.10.16](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.15...sn_transfers-v0.10.16) - 2023-07-25

### Other
- updated the following local packages: sn_protocol

## [0.10.15](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.14...sn_transfers-v0.10.15) - 2023-07-21

### Other
- updated the following local packages: sn_protocol

## [0.10.14](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.13...sn_transfers-v0.10.14) - 2023-07-20

### Other
- updated the following local packages: sn_protocol

## [0.10.13](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.12...sn_transfers-v0.10.13) - 2023-07-19

### Added
- *(CI)* dbc verfication during network churning test

## [0.10.12](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.11...sn_transfers-v0.10.12) - 2023-07-19

### Other
- updated the following local packages: sn_protocol

## [0.10.11](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.10...sn_transfers-v0.10.11) - 2023-07-18

### Other
- updated the following local packages: sn_protocol

## [0.10.10](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.9...sn_transfers-v0.10.10) - 2023-07-17

### Other
- updated the following local packages: sn_protocol

## [0.10.9](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.8...sn_transfers-v0.10.9) - 2023-07-17

### Added
- *(client)* keep storage payment proofs in local wallet

## [0.10.8](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.7...sn_transfers-v0.10.8) - 2023-07-12

### Other
- updated the following local packages: sn_protocol

## [0.10.7](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.6...sn_transfers-v0.10.7) - 2023-07-11

### Other
- updated the following local packages: sn_protocol

## [0.10.6](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.5...sn_transfers-v0.10.6) - 2023-07-10

### Other
- updated the following local packages: sn_protocol

## [0.10.5](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.4...sn_transfers-v0.10.5) - 2023-07-06

### Other
- updated the following local packages: sn_protocol

## [0.10.4](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.3...sn_transfers-v0.10.4) - 2023-07-05

### Other
- updated the following local packages: sn_protocol

## [0.10.3](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.2...sn_transfers-v0.10.3) - 2023-07-04

### Other
- updated the following local packages: sn_protocol

## [0.10.2](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.1...sn_transfers-v0.10.2) - 2023-06-28

### Other
- updated the following local packages: sn_protocol

## [0.10.1](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.10.0...sn_transfers-v0.10.1) - 2023-06-26

### Added
- display path when no deposits were found upon wallet deposit failure

### Other
- adding proptests for payment proofs merkletree utilities
- payment proof map to use xorname as index instead of merkletree nodes type
- having the payment proof validation util to return the item's leaf index

## [0.10.0](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.9.8...sn_transfers-v0.10.0) - 2023-06-22

### Added
- use standarised directories for files/wallet commands

## [0.9.8](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.9.7...sn_transfers-v0.9.8) - 2023-06-21

### Other
- updated the following local packages: sn_protocol

## [0.9.7](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.9.6...sn_transfers-v0.9.7) - 2023-06-21

### Fixed
- *(sn_transfers)* hardcode new genesis DBC for tests

### Other
- *(node)* obtain parent_tx from SignedSpend

## [0.9.6](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.9.5...sn_transfers-v0.9.6) - 2023-06-20

### Other
- updated the following local packages: sn_protocol

## [0.9.5](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.9.4...sn_transfers-v0.9.5) - 2023-06-20

### Other
- specific error types for different payment proof verification scenarios

## [0.9.4](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.9.3...sn_transfers-v0.9.4) - 2023-06-15

### Added
- add double spend test

### Fixed
- parent spend checks
- parent spend issue

## [0.9.3](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.9.2...sn_transfers-v0.9.3) - 2023-06-14

### Added
- include output DBC within payment proof for Chunks storage

## [0.9.2](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.9.1...sn_transfers-v0.9.2) - 2023-06-12

### Added
- remove spendbook rw locks, improve logging

## [0.9.1](https://github.com/maidsafe/safe_network/compare/sn_transfers-v0.9.0...sn_transfers-v0.9.1) - 2023-06-09

### Other
- manually change crate version
