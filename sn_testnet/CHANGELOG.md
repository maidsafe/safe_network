# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## v0.1.5 (2023-05-04)

### Chore

 - <csr-id-c5b3c83c771cdc44cf304ea50b1fcc1586854072/> disable some testnet verfications and add Cargo.lock to version control
 - <csr-id-1457a453341e35ad3fbf426b4e1fa4a57a753761/> ensure testnet launch fails if build fails
 - <csr-id-de04d62f6dc155616c14e0f4a07f3b8205398b1b/> remove deps, remove EnvFilter
   We can specify log levels in the code as needed without having to bring in
   EnvFilter (and regex).
   
   Although right now regex is used elsewhere, we can hopefully remove that large dep
 - <csr-id-d748fcd6e6c3ba604fb898b3be8b73e96270e993/> fix naming
 - <csr-id-ba7c74175e7082f6a2d4afc64a85be2c56b9d8c9/> add docs + clippy fixes
 - <csr-id-f772949320519c868a5e2ffc3b611aa138567afd/> cargo fix

### New Features

 - <csr-id-a9e6906a4dfabe389a242afbe472bc7c87427b19/> update the user when nodes verification starts
 - <csr-id-7859c5ee7650ff26b2a1e7b7770aaee1af5692db/> compare nodes logs info with the info retrieved from their RPC service
 - <csr-id-5b266b8bbd1f46d8b87917d0573377ff1ecaf2f7/> exposing a gRPC interface on safenode bin/app
 - <csr-id-5ce1e89c56cebd9c61f8032c2ca86c258e5f033a/> make req/resp generic
 - <csr-id-514e8153bfc33cd5bb12e7998dd065e5f5c30c4c/> add some logging to dirs per node
 - <csr-id-e7f1da121e9b7afd2784caeab1fd8b826c47fa85/> use a random port @ startup, write config if none exists
 - <csr-id-fa4b3eacb4930749ad229cf2dbd26949b0a77a7e/> initial copy of testnet bin with basic tweaks.

### Bug Fixes

 - <csr-id-cf9a375790770deb31d88515204d09becb3c89c7/> it was reporting redundant info if it was spanned in more than one log files pere node
 - <csr-id-18241f6b280f460812acd743b601ad3c4cce5212/> add root dir to node startup
 - <csr-id-892c8b3abf332fbbe100bf04c0b04cc9e67be828/> add env filter and strip back testnet bin
 - <csr-id-5e633868773e42c13326c2f52790c94d4cd88ae0/> clippy lints
 - <csr-id-6190d222e04904baad12070f3893c2d0c425238a/> initial comms by writing 127.0.0.1 ip addre for genesis

### Test

 - <csr-id-d8fc275020bdff5c0d555ae0d0dcd59c3d63a65c/> CI network churning test

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 22 commits contributed to the release over the course of 41 calendar days.
 - 19 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - It was reporting redundant info if it was spanned in more than one log files pere node ([`cf9a375`](https://github.com/maidsafe/safe_network/commit/cf9a375790770deb31d88515204d09becb3c89c7))
    - Disable some testnet verfications and add Cargo.lock to version control ([`c5b3c83`](https://github.com/maidsafe/safe_network/commit/c5b3c83c771cdc44cf304ea50b1fcc1586854072))
    - Update the user when nodes verification starts ([`a9e6906`](https://github.com/maidsafe/safe_network/commit/a9e6906a4dfabe389a242afbe472bc7c87427b19))
    - Compare nodes logs info with the info retrieved from their RPC service ([`7859c5e`](https://github.com/maidsafe/safe_network/commit/7859c5ee7650ff26b2a1e7b7770aaee1af5692db))
    - Add root dir to node startup ([`18241f6`](https://github.com/maidsafe/safe_network/commit/18241f6b280f460812acd743b601ad3c4cce5212))
    - CI network churning test ([`d8fc275`](https://github.com/maidsafe/safe_network/commit/d8fc275020bdff5c0d555ae0d0dcd59c3d63a65c))
    - Exposing a gRPC interface on safenode bin/app ([`5b266b8`](https://github.com/maidsafe/safe_network/commit/5b266b8bbd1f46d8b87917d0573377ff1ecaf2f7))
    - Ensure testnet launch fails if build fails ([`1457a45`](https://github.com/maidsafe/safe_network/commit/1457a453341e35ad3fbf426b4e1fa4a57a753761))
    - Remove deps, remove EnvFilter ([`de04d62`](https://github.com/maidsafe/safe_network/commit/de04d62f6dc155616c14e0f4a07f3b8205398b1b))
    - Fix naming ([`d748fcd`](https://github.com/maidsafe/safe_network/commit/d748fcd6e6c3ba604fb898b3be8b73e96270e993))
    - Add docs + clippy fixes ([`ba7c741`](https://github.com/maidsafe/safe_network/commit/ba7c74175e7082f6a2d4afc64a85be2c56b9d8c9))
    - Make req/resp generic ([`5ce1e89`](https://github.com/maidsafe/safe_network/commit/5ce1e89c56cebd9c61f8032c2ca86c258e5f033a))
    - Add env filter and strip back testnet bin ([`892c8b3`](https://github.com/maidsafe/safe_network/commit/892c8b3abf332fbbe100bf04c0b04cc9e67be828))
    - Clippy lints ([`5e63386`](https://github.com/maidsafe/safe_network/commit/5e633868773e42c13326c2f52790c94d4cd88ae0))
    - 25 nodes and a couple of searches ([`1a22722`](https://github.com/maidsafe/safe_network/commit/1a22722198b5aecaca00dc167c7084d06f39160b))
    - Merge pull request #8 from joshuef/RandomPortNodes ([`34b2bfb`](https://github.com/maidsafe/safe_network/commit/34b2bfb7746fcd16f08aa2431181a502135b2865))
    - Initial comms by writing 127.0.0.1 ip addre for genesis ([`6190d22`](https://github.com/maidsafe/safe_network/commit/6190d222e04904baad12070f3893c2d0c425238a))
    - Add some logging to dirs per node ([`514e815`](https://github.com/maidsafe/safe_network/commit/514e8153bfc33cd5bb12e7998dd065e5f5c30c4c))
    - Cargo fix ([`f772949`](https://github.com/maidsafe/safe_network/commit/f772949320519c868a5e2ffc3b611aa138567afd))
    - Use a random port @ startup, write config if none exists ([`e7f1da1`](https://github.com/maidsafe/safe_network/commit/e7f1da121e9b7afd2784caeab1fd8b826c47fa85))
    - Merge pull request #6 from joshuef/AddTestnetBin ([`874c014`](https://github.com/maidsafe/safe_network/commit/874c01401acf980a226839247514e4bd69a58273))
    - Initial copy of testnet bin with basic tweaks. ([`fa4b3ea`](https://github.com/maidsafe/safe_network/commit/fa4b3eacb4930749ad229cf2dbd26949b0a77a7e))
</details>

## v0.1.4 (2023-03-23)

### New Features

 - <csr-id-16bb3389cdd665fe9a577587d9b7a6e8d21a3028/> exposing a gRPC interface on safenode bin/app
   - The safenode RPC service is exposed only when built with 'rpc-service' feature.
- The safenode RPC service code is generated automatically using gRPC (`tonic` crate)
   from a `proto` file with messages definitions added to sn_interface.
- The RPC is exposed at the same address as the node's address used for network connections,
   but using the subsequent port number.
- A new final step was implemented for the sn_testnet tool, to run a check on the launched nodes,
   verifying their names and network knowledge are the expected for the launched testnet.
- The new sn_testnet tool step is run only if built with 'verify-nodes' feature.
- Running the `verify-nodes` check of sn_testnet in CI previous to sn_client e2e tests.

## v0.1.3 (2023-03-22)

<csr-id-b0627339e2458fd762084cc4805d7adedfd8c05e/>
<csr-id-c9f3e7ccad8836c609193f1c6b53f351e5705805/>
<csr-id-50f6ede2104025bd79de8922ca7f27c742cf52bb/>
<csr-id-807d69ef609decfe94230e2086144afc5cc56d7b/>
<csr-id-1a8b9c9ba5b98c0f1176a0ccbce53d4acea8c84c/>
<csr-id-d3c6c9727a69389f4204b746c54a537cd783232c/>
<csr-id-22c6e341d28c913a3acaaeae0ceeb8c0a1ef4d4e/>

### Chore

 - <csr-id-b0627339e2458fd762084cc4805d7adedfd8c05e/> sn_testnet-0.1.3/sn_interface-0.20.7/sn_comms-0.6.4/sn_client-0.82.4/sn_node-0.80.1/sn_api-0.80.3/sn_cli-0.74.2
 - <csr-id-c9f3e7ccad8836c609193f1c6b53f351e5705805/> sn_node-0.80.0
 - <csr-id-50f6ede2104025bd79de8922ca7f27c742cf52bb/> sn_interface-0.20.6/sn_comms-0.6.3/sn_client-0.82.3/sn_node-0.79.0/sn_cli-0.74.1
 - <csr-id-807d69ef609decfe94230e2086144afc5cc56d7b/> sn_interface-0.20.6/sn_comms-0.6.3/sn_client-0.82.3/sn_node-0.79.0/sn_cli-0.74.1
 - <csr-id-1a8b9c9ba5b98c0f1176a0ccbce53d4acea8c84c/> safenode renaming

### Chore

 - <csr-id-22c6e341d28c913a3acaaeae0ceeb8c0a1ef4d4e/> sn_testnet-0.1.3/sn_interface-0.20.7/sn_comms-0.6.4/sn_client-0.82.4/sn_node-0.80.1/sn_api-0.80.3/sn_cli-0.74.2

### Refactor

 - <csr-id-d3c6c9727a69389f4204b746c54a537cd783232c/> remove unused wiremsg-debuginfo ft

## v0.1.2 (2023-03-16)

<csr-id-50f6ede2104025bd79de8922ca7f27c742cf52bb/>
<csr-id-807d69ef609decfe94230e2086144afc5cc56d7b/>
<csr-id-1a8b9c9ba5b98c0f1176a0ccbce53d4acea8c84c/>

### Chore

 - <csr-id-50f6ede2104025bd79de8922ca7f27c742cf52bb/> sn_interface-0.20.6/sn_comms-0.6.3/sn_client-0.82.3/sn_node-0.79.0/sn_cli-0.74.1
 - <csr-id-807d69ef609decfe94230e2086144afc5cc56d7b/> sn_interface-0.20.6/sn_comms-0.6.3/sn_client-0.82.3/sn_node-0.79.0/sn_cli-0.74.1
 - <csr-id-1a8b9c9ba5b98c0f1176a0ccbce53d4acea8c84c/> safenode renaming

## v0.1.1 (2023-03-16)

<csr-id-807d69ef609decfe94230e2086144afc5cc56d7b/>
<csr-id-1a8b9c9ba5b98c0f1176a0ccbce53d4acea8c84c/>

### Chore

 - <csr-id-807d69ef609decfe94230e2086144afc5cc56d7b/> sn_interface-0.20.6/sn_comms-0.6.3/sn_client-0.82.3/sn_node-0.79.0/sn_cli-0.74.1
 - <csr-id-1a8b9c9ba5b98c0f1176a0ccbce53d4acea8c84c/> safenode renaming

## [0.1.6](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.5...sn_testnet-v0.1.6) - 2023-06-08

### Other
- update dependencies

## [0.1.7](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.6...sn_testnet-v0.1.7) - 2023-06-09

### Other
- provide clarity on command arguments

## [0.1.8](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.7...sn_testnet-v0.1.8) - 2023-06-09

### Other
- update dependencies

## [0.1.9](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.8...sn_testnet-v0.1.9) - 2023-06-09

### Other
- update dependencies

## [0.1.10](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.9...sn_testnet-v0.1.10) - 2023-06-09

### Other
- update dependencies

## [0.1.11](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.10...sn_testnet-v0.1.11) - 2023-06-09

### Other
- improve documentation for cli commands

## [0.1.12](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.11...sn_testnet-v0.1.12) - 2023-06-12

### Other
- update dependencies

## [0.1.13](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.12...sn_testnet-v0.1.13) - 2023-06-12

### Other
- update dependencies

## [0.1.14](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.13...sn_testnet-v0.1.14) - 2023-06-13

### Other
- update dependencies

## [0.1.15](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.14...sn_testnet-v0.1.15) - 2023-06-13

### Other
- update dependencies

## [0.1.16](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.15...sn_testnet-v0.1.16) - 2023-06-14

### Other
- update dependencies

## [0.1.17](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.16...sn_testnet-v0.1.17) - 2023-06-14

### Other
- update dependencies

## [0.1.18](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.17...sn_testnet-v0.1.18) - 2023-06-14

### Other
- update dependencies

## [0.1.19](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.18...sn_testnet-v0.1.19) - 2023-06-14

### Other
- update dependencies

## [0.1.20](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.19...sn_testnet-v0.1.20) - 2023-06-15

### Other
- update dependencies

## [0.1.21](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.20...sn_testnet-v0.1.21) - 2023-06-15

### Other
- update dependencies

## [0.1.22](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.21...sn_testnet-v0.1.22) - 2023-06-15

### Other
- update dependencies

## [0.1.23](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.22...sn_testnet-v0.1.23) - 2023-06-15

### Other
- update dependencies

## [0.1.24](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.23...sn_testnet-v0.1.24) - 2023-06-15

### Other
- update dependencies

## [0.1.25](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.24...sn_testnet-v0.1.25) - 2023-06-15

### Other
- update dependencies

## [0.1.26](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.25...sn_testnet-v0.1.26) - 2023-06-15

### Other
- update dependencies

## [0.1.27](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.26...sn_testnet-v0.1.27) - 2023-06-16

### Other
- update dependencies

## [0.1.28](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.27...sn_testnet-v0.1.28) - 2023-06-16

### Other
- update dependencies

## [0.1.29](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.28...sn_testnet-v0.1.29) - 2023-06-16

### Other
- update dependencies

## [0.1.30](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.29...sn_testnet-v0.1.30) - 2023-06-16

### Other
- update dependencies

## [0.1.31](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.30...sn_testnet-v0.1.31) - 2023-06-16

### Other
- update dependencies

## [0.1.32](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.31...sn_testnet-v0.1.32) - 2023-06-16

### Other
- update dependencies

## [0.1.33](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.32...sn_testnet-v0.1.33) - 2023-06-16

### Other
- update dependencies

## [0.1.34](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.33...sn_testnet-v0.1.34) - 2023-06-19

### Other
- update dependencies

## [0.1.35](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.34...sn_testnet-v0.1.35) - 2023-06-19

### Other
- update dependencies

## [0.1.36](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.35...sn_testnet-v0.1.36) - 2023-06-19

### Other
- update dependencies

## [0.1.37](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.36...sn_testnet-v0.1.37) - 2023-06-19

### Other
- update dependencies

## [0.1.38](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.37...sn_testnet-v0.1.38) - 2023-06-19

### Other
- update dependencies

## [0.1.39](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.38...sn_testnet-v0.1.39) - 2023-06-19

### Other
- update dependencies

## [0.1.40](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.39...sn_testnet-v0.1.40) - 2023-06-20

### Other
- update dependencies

## [0.1.41](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.40...sn_testnet-v0.1.41) - 2023-06-20

### Other
- update dependencies

## [0.1.42](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.41...sn_testnet-v0.1.42) - 2023-06-20

### Other
- update dependencies

## [0.1.43](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.42...sn_testnet-v0.1.43) - 2023-06-20

### Other
- update dependencies

## [0.1.44](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.43...sn_testnet-v0.1.44) - 2023-06-20

### Other
- update dependencies

## [0.1.45](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.44...sn_testnet-v0.1.45) - 2023-06-20

### Other
- update dependencies

## [0.1.46](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.45...sn_testnet-v0.1.46) - 2023-06-21

### Other
- update dependencies

## [0.1.47](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.46...sn_testnet-v0.1.47) - 2023-06-21

### Other
- update dependencies

## [0.1.48](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.47...sn_testnet-v0.1.48) - 2023-06-21

### Other
- update dependencies

## [0.1.49](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.48...sn_testnet-v0.1.49) - 2023-06-21

### Other
- update dependencies

## [0.1.50](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.49...sn_testnet-v0.1.50) - 2023-06-22

### Other
- update dependencies

## [0.1.51](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.50...sn_testnet-v0.1.51) - 2023-06-22

### Other
- update dependencies

## [0.1.52](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.51...sn_testnet-v0.1.52) - 2023-06-22

### Other
- update dependencies

## [0.1.53](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.52...sn_testnet-v0.1.53) - 2023-06-23

### Other
- update dependencies

## [0.1.54](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.53...sn_testnet-v0.1.54) - 2023-06-23

### Other
- update dependencies

## [0.1.55](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.54...sn_testnet-v0.1.55) - 2023-06-23

### Other
- update dependencies

## [0.1.56](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.55...sn_testnet-v0.1.56) - 2023-06-23

### Other
- update dependencies

## [0.1.57](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.56...sn_testnet-v0.1.57) - 2023-06-24

### Other
- update dependencies

## [0.1.58](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.57...sn_testnet-v0.1.58) - 2023-06-26

### Other
- update dependencies

## [0.1.59](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.58...sn_testnet-v0.1.59) - 2023-06-26

### Other
- update dependencies

## [0.1.60](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.59...sn_testnet-v0.1.60) - 2023-06-26

### Other
- update dependencies

## [0.1.61](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.60...sn_testnet-v0.1.61) - 2023-06-26

### Other
- update dependencies

## [0.1.62](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.61...sn_testnet-v0.1.62) - 2023-06-26

### Other
- update dependencies

## [0.1.63](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.62...sn_testnet-v0.1.63) - 2023-06-27

### Other
- update dependencies

## [0.1.64](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.63...sn_testnet-v0.1.64) - 2023-06-27

### Other
- update dependencies

## [0.1.65](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.64...sn_testnet-v0.1.65) - 2023-06-27

### Other
- update dependencies

## [0.1.66](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.65...sn_testnet-v0.1.66) - 2023-06-28

### Other
- update dependencies

## [0.1.67](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.66...sn_testnet-v0.1.67) - 2023-06-28

### Other
- update dependencies

## [0.1.68](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.67...sn_testnet-v0.1.68) - 2023-06-28

### Other
- update dependencies

## [0.1.69](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.68...sn_testnet-v0.1.69) - 2023-06-28

### Other
- update dependencies

## [0.1.70](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.69...sn_testnet-v0.1.70) - 2023-06-29

### Other
- update dependencies

## [0.1.71](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.70...sn_testnet-v0.1.71) - 2023-06-29

### Other
- update dependencies

## [0.1.72](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.71...sn_testnet-v0.1.72) - 2023-07-03

### Other
- update dependencies

## [0.1.73](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.72...sn_testnet-v0.1.73) - 2023-07-04

### Other
- update dependencies

## [0.1.74](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.73...sn_testnet-v0.1.74) - 2023-07-05

### Other
- update dependencies

## [0.1.75](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.74...sn_testnet-v0.1.75) - 2023-07-05

### Other
- update dependencies

## [0.1.76](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.75...sn_testnet-v0.1.76) - 2023-07-06

### Other
- update benchmark workflows for new directories
- update node logging paths

## [0.1.77](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.76...sn_testnet-v0.1.77) - 2023-07-06

### Other
- update dependencies

## [0.1.78](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.77...sn_testnet-v0.1.78) - 2023-07-06

### Other
- update dependencies

## [0.2.0](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.1.78...sn_testnet-v0.2.0) - 2023-07-07

### Added
- provide a `--clean` flag
- remove node directory management
- remove network contacts from `testnet` bin

### Other
- restore sn_testnet unit tests
- obtain genesis peer id directly

## [0.2.1](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.0...sn_testnet-v0.2.1) - 2023-07-07

### Other
- update dependencies

## [0.2.2](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.1...sn_testnet-v0.2.2) - 2023-07-10

### Other
- update dependencies

## [0.2.3](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.2...sn_testnet-v0.2.3) - 2023-07-10

### Other
- update dependencies

## [0.2.4](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.3...sn_testnet-v0.2.4) - 2023-07-10

### Other
- update dependencies

## [0.2.5](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.4...sn_testnet-v0.2.5) - 2023-07-10

### Added
- *(testnet)* dont throw if no node files to clean

## [0.2.6](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.5...sn_testnet-v0.2.6) - 2023-07-11

### Other
- update dependencies

## [0.2.7](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.6...sn_testnet-v0.2.7) - 2023-07-11

### Other
- update dependencies

## [0.2.8](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.7...sn_testnet-v0.2.8) - 2023-07-11

### Other
- update dependencies

## [0.2.9](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.8...sn_testnet-v0.2.9) - 2023-07-11

### Other
- update dependencies

## [0.2.10](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.9...sn_testnet-v0.2.10) - 2023-07-12

### Other
- update dependencies

## [0.2.11](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.10...sn_testnet-v0.2.11) - 2023-07-13

### Other
- update dependencies

## [0.2.12](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.11...sn_testnet-v0.2.12) - 2023-07-13

### Other
- update dependencies

## [0.2.13](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.12...sn_testnet-v0.2.13) - 2023-07-17

### Other
- update dependencies

## [0.2.14](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.13...sn_testnet-v0.2.14) - 2023-07-17

### Added
- *(networking)* upgrade to libp2p 0.52.0

## [0.2.15](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.14...sn_testnet-v0.2.15) - 2023-07-17

### Other
- update dependencies

## [0.2.16](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.15...sn_testnet-v0.2.16) - 2023-07-17

### Other
- update dependencies

## [0.2.17](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.16...sn_testnet-v0.2.17) - 2023-07-18

### Other
- update dependencies

## [0.2.18](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.17...sn_testnet-v0.2.18) - 2023-07-18

### Other
- update dependencies

## [0.2.19](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.18...sn_testnet-v0.2.19) - 2023-07-18

### Other
- update dependencies

## [0.2.20](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.19...sn_testnet-v0.2.20) - 2023-07-18

### Other
- update dependencies

## [0.2.21](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.20...sn_testnet-v0.2.21) - 2023-07-19

### Other
- update dependencies

## [0.2.22](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.21...sn_testnet-v0.2.22) - 2023-07-19

### Added
- faucet integration in testnet bin

## [0.2.23](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.22...sn_testnet-v0.2.23) - 2023-07-19

### Other
- update dependencies

## [0.2.24](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.23...sn_testnet-v0.2.24) - 2023-07-19

### Added
- *(testnet)* enable the use of `CARGO_TARGET_DIR`

## [0.2.25](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.24...sn_testnet-v0.2.25) - 2023-07-20

### Other
- update dependencies

## [0.2.26](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.25...sn_testnet-v0.2.26) - 2023-07-20

### Other
- update dependencies

## [0.2.27](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.26...sn_testnet-v0.2.27) - 2023-07-21

### Other
- update dependencies

## [0.2.28](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.27...sn_testnet-v0.2.28) - 2023-07-25

### Other
- update dependencies

## [0.2.29](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.28...sn_testnet-v0.2.29) - 2023-07-26

### Other
- *(testnet)* always start the faucet

## [0.2.30](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.29...sn_testnet-v0.2.30) - 2023-07-26

### Other
- update dependencies

## [0.2.31](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.30...sn_testnet-v0.2.31) - 2023-07-26

### Added
- *(testnet)* provide args to build/run faucet

## [0.2.32](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.31...sn_testnet-v0.2.32) - 2023-07-26

### Other
- update dependencies

## [0.2.33](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.32...sn_testnet-v0.2.33) - 2023-07-26

### Other
- update dependencies

## [0.2.34](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.33...sn_testnet-v0.2.34) - 2023-07-26

### Other
- update dependencies

## [0.2.35](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.34...sn_testnet-v0.2.35) - 2023-07-27

### Other
- update dependencies

## [0.2.36](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.35...sn_testnet-v0.2.36) - 2023-07-28

### Other
- update dependencies

## [0.2.37](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.36...sn_testnet-v0.2.37) - 2023-07-28

### Other
- *(testnet)* build only the provided binaries

## [0.2.38](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.37...sn_testnet-v0.2.38) - 2023-07-28

### Other
- update dependencies

## [0.2.39](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.38...sn_testnet-v0.2.39) - 2023-07-28

### Other
- update dependencies

## [0.2.40](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.39...sn_testnet-v0.2.40) - 2023-07-31

### Other
- update dependencies

## [0.2.41](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.40...sn_testnet-v0.2.41) - 2023-07-31

### Other
- update dependencies

## [0.2.42](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.41...sn_testnet-v0.2.42) - 2023-07-31

### Other
- update dependencies

## [0.2.43](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.42...sn_testnet-v0.2.43) - 2023-07-31

### Other
- update dependencies

## [0.2.44](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.43...sn_testnet-v0.2.44) - 2023-08-01

### Other
- update dependencies

## [0.2.45](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.44...sn_testnet-v0.2.45) - 2023-08-01

### Other
- update dependencies

## [0.2.46](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.45...sn_testnet-v0.2.46) - 2023-08-01

### Other
- update dependencies

## [0.2.47](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.46...sn_testnet-v0.2.47) - 2023-08-01

### Other
- update dependencies

## [0.2.48](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.47...sn_testnet-v0.2.48) - 2023-08-01

### Other
- update dependencies

## [0.2.49](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.48...sn_testnet-v0.2.49) - 2023-08-01

### Other
- update dependencies

## [0.2.50](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.49...sn_testnet-v0.2.50) - 2023-08-02

### Other
- update dependencies

## [0.2.51](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.50...sn_testnet-v0.2.51) - 2023-08-02

### Fixed
- waiting to allow faucet server to be settled

## [0.2.52](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.51...sn_testnet-v0.2.52) - 2023-08-03

### Other
- update dependencies

## [0.2.53](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.52...sn_testnet-v0.2.53) - 2023-08-03

### Other
- *(testnet)* provide faucet log arg

## [0.2.54](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.53...sn_testnet-v0.2.54) - 2023-08-03

### Other
- update dependencies

## [0.2.55](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.54...sn_testnet-v0.2.55) - 2023-08-03

### Other
- reduce the wait after create faucet server

## [0.2.56](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.55...sn_testnet-v0.2.56) - 2023-08-03

### Other
- update dependencies

## [0.2.57](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.56...sn_testnet-v0.2.57) - 2023-08-04

### Other
- update dependencies

## [0.2.58](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.57...sn_testnet-v0.2.58) - 2023-08-04

### Other
- update dependencies

## [0.2.59](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.58...sn_testnet-v0.2.59) - 2023-08-07

### Other
- update dependencies

## [0.2.60](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.59...sn_testnet-v0.2.60) - 2023-08-07

### Other
- update dependencies

## [0.2.61](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.60...sn_testnet-v0.2.61) - 2023-08-07

### Other
- update dependencies

## [0.2.62](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.61...sn_testnet-v0.2.62) - 2023-08-07

### Other
- update dependencies

## [0.2.63](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.62...sn_testnet-v0.2.63) - 2023-08-08

### Other
- update dependencies

## [0.2.64](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.63...sn_testnet-v0.2.64) - 2023-08-09

### Fixed
- *(testnet)* provide bootstrap peer when launching faucet

## [0.2.65](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.64...sn_testnet-v0.2.65) - 2023-08-10

### Other
- update dependencies

## [0.2.66](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.65...sn_testnet-v0.2.66) - 2023-08-10

### Other
- update dependencies

## [0.2.67](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.66...sn_testnet-v0.2.67) - 2023-08-11

### Other
- update dependencies

## [0.2.68](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.67...sn_testnet-v0.2.68) - 2023-08-11

### Other
- update dependencies

## [0.2.69](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.68...sn_testnet-v0.2.69) - 2023-08-14

### Other
- update dependencies

## [0.2.70](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.69...sn_testnet-v0.2.70) - 2023-08-14

### Other
- update dependencies

## [0.2.71](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.70...sn_testnet-v0.2.71) - 2023-08-15

### Other
- update dependencies

## [0.2.72](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.71...sn_testnet-v0.2.72) - 2023-08-16

### Other
- update dependencies

## [0.2.73](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.72...sn_testnet-v0.2.73) - 2023-08-16

### Other
- update dependencies

## [0.2.74](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.73...sn_testnet-v0.2.74) - 2023-08-16

### Other
- update dependencies

## [0.2.75](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.74...sn_testnet-v0.2.75) - 2023-08-17

### Other
- update dependencies

## [0.2.76](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.75...sn_testnet-v0.2.76) - 2023-08-17

### Other
- update dependencies

## [0.2.77](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.76...sn_testnet-v0.2.77) - 2023-08-17

### Other
- update dependencies

## [0.2.78](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.77...sn_testnet-v0.2.78) - 2023-08-17

### Other
- update dependencies

## [0.2.79](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.78...sn_testnet-v0.2.79) - 2023-08-18

### Other
- update dependencies

## [0.2.80](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.79...sn_testnet-v0.2.80) - 2023-08-18

### Other
- update dependencies

## [0.2.81](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.80...sn_testnet-v0.2.81) - 2023-08-21

### Other
- update dependencies

## [0.2.82](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.81...sn_testnet-v0.2.82) - 2023-08-21

### Other
- update dependencies

## [0.2.83](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.82...sn_testnet-v0.2.83) - 2023-08-22

### Other
- update dependencies

## [0.2.84](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.83...sn_testnet-v0.2.84) - 2023-08-22

### Other
- update dependencies

## [0.2.85](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.84...sn_testnet-v0.2.85) - 2023-08-24

### Other
- update dependencies

## [0.2.86](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.85...sn_testnet-v0.2.86) - 2023-08-24

### Other
- update dependencies

## [0.2.87](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.86...sn_testnet-v0.2.87) - 2023-08-24

### Other
- update dependencies

## [0.2.88](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.87...sn_testnet-v0.2.88) - 2023-08-25

### Other
- update dependencies

## [0.2.89](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.88...sn_testnet-v0.2.89) - 2023-08-29

### Added
- *(node)* add feature flag for tcp/quic

## [0.2.90](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.89...sn_testnet-v0.2.90) - 2023-08-30

### Other
- update dependencies

## [0.2.91](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.90...sn_testnet-v0.2.91) - 2023-08-30

### Other
- update dependencies

## [0.2.92](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.91...sn_testnet-v0.2.92) - 2023-08-30

### Other
- *(node)* data verification log tweaks

## [0.2.93](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.92...sn_testnet-v0.2.93) - 2023-08-31

### Other
- update dependencies

## [0.2.94](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.93...sn_testnet-v0.2.94) - 2023-08-31

### Other
- update dependencies

## [0.2.95](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.94...sn_testnet-v0.2.95) - 2023-08-31

### Other
- update dependencies

## [0.2.96](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.95...sn_testnet-v0.2.96) - 2023-08-31

### Other
- update dependencies

## [0.2.97](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.96...sn_testnet-v0.2.97) - 2023-08-31

### Other
- update dependencies

## [0.2.98](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.97...sn_testnet-v0.2.98) - 2023-08-31

### Other
- update dependencies

## [0.2.99](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.98...sn_testnet-v0.2.99) - 2023-08-31

### Other
- update dependencies

## [0.2.100](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.99...sn_testnet-v0.2.100) - 2023-08-31

### Other
- remove unused async

## [0.2.101](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.100...sn_testnet-v0.2.101) - 2023-09-01

### Other
- update dependencies

## [0.2.102](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.101...sn_testnet-v0.2.102) - 2023-09-01

### Other
- update dependencies

## [0.2.103](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.102...sn_testnet-v0.2.103) - 2023-09-01

### Other
- update dependencies

## [0.2.104](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.103...sn_testnet-v0.2.104) - 2023-09-01

### Other
- update dependencies

## [0.2.105](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.104...sn_testnet-v0.2.105) - 2023-09-02

### Other
- update dependencies

## [0.2.106](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.105...sn_testnet-v0.2.106) - 2023-09-04

### Other
- update dependencies

## [0.2.107](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.106...sn_testnet-v0.2.107) - 2023-09-04

### Other
- update dependencies

## [0.2.108](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.107...sn_testnet-v0.2.108) - 2023-09-04

### Other
- update dependencies

## [0.2.109](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.108...sn_testnet-v0.2.109) - 2023-09-05

### Other
- update dependencies

## [0.2.110](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.109...sn_testnet-v0.2.110) - 2023-09-05

### Other
- update dependencies

## [0.2.111](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.110...sn_testnet-v0.2.111) - 2023-09-05

### Other
- update dependencies

## [0.2.112](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.111...sn_testnet-v0.2.112) - 2023-09-05

### Other
- update dependencies

## [0.2.113](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.112...sn_testnet-v0.2.113) - 2023-09-05

### Other
- update dependencies

## [0.2.114](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.113...sn_testnet-v0.2.114) - 2023-09-06

### Other
- update dependencies

## [0.2.115](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.114...sn_testnet-v0.2.115) - 2023-09-07

### Other
- update dependencies

## [0.2.116](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.115...sn_testnet-v0.2.116) - 2023-09-07

### Other
- update dependencies

## [0.2.117](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.116...sn_testnet-v0.2.117) - 2023-09-07

### Other
- update dependencies

## [0.2.118](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.117...sn_testnet-v0.2.118) - 2023-09-08

### Other
- update dependencies

## [0.2.119](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.118...sn_testnet-v0.2.119) - 2023-09-11

### Other
- update dependencies

## [0.2.120](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.119...sn_testnet-v0.2.120) - 2023-09-11

### Other
- update dependencies

## [0.2.121](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.120...sn_testnet-v0.2.121) - 2023-09-11

### Other
- update dependencies

## [0.2.122](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.121...sn_testnet-v0.2.122) - 2023-09-12

### Other
- update dependencies

## [0.2.123](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.122...sn_testnet-v0.2.123) - 2023-09-12

### Other
- *(metrics)* rename network metrics and remove from default features list

## [0.2.124](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.123...sn_testnet-v0.2.124) - 2023-09-12

### Other
- update dependencies

## [0.2.125](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.124...sn_testnet-v0.2.125) - 2023-09-12

### Other
- update dependencies

## [0.2.126](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.125...sn_testnet-v0.2.126) - 2023-09-13

### Other
- update dependencies

## [0.2.127](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.126...sn_testnet-v0.2.127) - 2023-09-14

### Other
- *(metrics)* rename feature flag and small fixes

## [0.2.128](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.127...sn_testnet-v0.2.128) - 2023-09-14

### Other
- update dependencies

## [0.2.129](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.128...sn_testnet-v0.2.129) - 2023-09-14

### Other
- update dependencies

## [0.2.130](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.129...sn_testnet-v0.2.130) - 2023-09-15

### Other
- update dependencies

## [0.2.131](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.130...sn_testnet-v0.2.131) - 2023-09-15

### Other
- update dependencies

## [0.2.132](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.131...sn_testnet-v0.2.132) - 2023-09-15

### Other
- update dependencies

## [0.2.133](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.132...sn_testnet-v0.2.133) - 2023-09-15

### Other
- update dependencies

## [0.2.134](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.133...sn_testnet-v0.2.134) - 2023-09-18

### Other
- update dependencies

## [0.2.135](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.134...sn_testnet-v0.2.135) - 2023-09-18

### Other
- update dependencies

## [0.2.136](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.135...sn_testnet-v0.2.136) - 2023-09-18

### Other
- update dependencies

## [0.2.137](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.136...sn_testnet-v0.2.137) - 2023-09-18

### Other
- update dependencies

## [0.2.138](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.137...sn_testnet-v0.2.138) - 2023-09-19

### Other
- update dependencies

## [0.2.139](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.138...sn_testnet-v0.2.139) - 2023-09-19

### Other
- update dependencies

## [0.2.140](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.139...sn_testnet-v0.2.140) - 2023-09-19

### Other
- update dependencies

## [0.2.141](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.140...sn_testnet-v0.2.141) - 2023-09-19

### Other
- update dependencies

## [0.2.142](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.141...sn_testnet-v0.2.142) - 2023-09-19

### Other
- update dependencies

## [0.2.143](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.142...sn_testnet-v0.2.143) - 2023-09-19

### Other
- update dependencies

## [0.2.144](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.143...sn_testnet-v0.2.144) - 2023-09-20

### Other
- update dependencies

## [0.2.145](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.144...sn_testnet-v0.2.145) - 2023-09-20

### Other
- major dep updates

## [0.2.146](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.145...sn_testnet-v0.2.146) - 2023-09-20

### Other
- update dependencies

## [0.2.147](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.146...sn_testnet-v0.2.147) - 2023-09-20

### Other
- update dependencies

## [0.2.148](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.147...sn_testnet-v0.2.148) - 2023-09-20

### Other
- update dependencies

## [0.2.149](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.148...sn_testnet-v0.2.149) - 2023-09-20

### Other
- update dependencies

## [0.2.150](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.149...sn_testnet-v0.2.150) - 2023-09-20

### Other
- update dependencies

## [0.2.151](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.150...sn_testnet-v0.2.151) - 2023-09-20

### Other
- update dependencies

## [0.2.152](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.151...sn_testnet-v0.2.152) - 2023-09-20

### Other
- update dependencies

## [0.2.153](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.152...sn_testnet-v0.2.153) - 2023-09-20

### Other
- update dependencies

## [0.2.154](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.153...sn_testnet-v0.2.154) - 2023-09-21

### Other
- update dependencies

## [0.2.155](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.154...sn_testnet-v0.2.155) - 2023-09-22

### Other
- update dependencies

## [0.2.156](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.155...sn_testnet-v0.2.156) - 2023-09-22

### Other
- update dependencies

## [0.2.157](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.156...sn_testnet-v0.2.157) - 2023-09-25

### Other
- update dependencies

## [0.2.158](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.157...sn_testnet-v0.2.158) - 2023-09-25

### Fixed
- *(peers)* node can start without bootstrap peers

## [0.2.159](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.158...sn_testnet-v0.2.159) - 2023-09-25

### Other
- update dependencies

## [0.2.160](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.159...sn_testnet-v0.2.160) - 2023-09-25

### Other
- update dependencies

## [0.2.161](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.160...sn_testnet-v0.2.161) - 2023-09-25

### Other
- update dependencies

## [0.2.162](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.161...sn_testnet-v0.2.162) - 2023-09-26

### Other
- update dependencies

## [0.2.163](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.162...sn_testnet-v0.2.163) - 2023-09-26

### Other
- update dependencies

## [0.2.164](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.163...sn_testnet-v0.2.164) - 2023-09-27

### Other
- update dependencies

## [0.2.165](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.164...sn_testnet-v0.2.165) - 2023-09-27

### Other
- update dependencies

## [0.2.166](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.165...sn_testnet-v0.2.166) - 2023-09-27

### Other
- update dependencies

## [0.2.167](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.166...sn_testnet-v0.2.167) - 2023-09-28

### Other
- update dependencies

## [0.2.168](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.167...sn_testnet-v0.2.168) - 2023-09-29

### Other
- update dependencies

## [0.2.169](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.168...sn_testnet-v0.2.169) - 2023-09-29

### Other
- update dependencies

## [0.2.170](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.169...sn_testnet-v0.2.170) - 2023-09-29

### Other
- update dependencies

## [0.2.171](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.170...sn_testnet-v0.2.171) - 2023-10-02

### Other
- update dependencies

## [0.2.172](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.171...sn_testnet-v0.2.172) - 2023-10-02

### Other
- update dependencies

## [0.2.173](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.172...sn_testnet-v0.2.173) - 2023-10-02

### Other
- update dependencies

## [0.2.174](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.173...sn_testnet-v0.2.174) - 2023-10-02

### Other
- update dependencies

## [0.2.175](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.174...sn_testnet-v0.2.175) - 2023-10-03

### Other
- update dependencies

## [0.2.176](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.175...sn_testnet-v0.2.176) - 2023-10-03

### Other
- update dependencies

## [0.2.177](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.176...sn_testnet-v0.2.177) - 2023-10-03

### Other
- update dependencies

## [0.2.178](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.177...sn_testnet-v0.2.178) - 2023-10-03

### Other
- update dependencies

## [0.2.179](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.178...sn_testnet-v0.2.179) - 2023-10-03

### Other
- update dependencies

## [0.2.180](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.179...sn_testnet-v0.2.180) - 2023-10-04

### Other
- update dependencies

## [0.2.181](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.180...sn_testnet-v0.2.181) - 2023-10-04

### Other
- update dependencies

## [0.2.182](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.181...sn_testnet-v0.2.182) - 2023-10-04

### Other
- update dependencies

## [0.2.183](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.182...sn_testnet-v0.2.183) - 2023-10-04

### Other
- update dependencies

## [0.2.184](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.183...sn_testnet-v0.2.184) - 2023-10-04

### Other
- update dependencies

## [0.2.185](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.184...sn_testnet-v0.2.185) - 2023-10-05

### Other
- update dependencies

## [0.2.186](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.185...sn_testnet-v0.2.186) - 2023-10-05

### Other
- update dependencies

## [0.2.187](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.186...sn_testnet-v0.2.187) - 2023-10-05

### Other
- update dependencies

## [0.2.188](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.187...sn_testnet-v0.2.188) - 2023-10-05

### Other
- update dependencies

## [0.2.189](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.188...sn_testnet-v0.2.189) - 2023-10-05

### Added
- *(metrics)* enable process memory and cpu usage metrics
- *(metrics)* enable node monitoring through dockerized grafana instance

### Fixed
- *(docs)* update metrics readme file

## [0.2.190](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.189...sn_testnet-v0.2.190) - 2023-10-06

### Other
- update dependencies

## [0.2.191](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.190...sn_testnet-v0.2.191) - 2023-10-06

### Other
- update dependencies

## [0.2.192](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.191...sn_testnet-v0.2.192) - 2023-10-06

### Other
- update dependencies

## [0.2.193](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.192...sn_testnet-v0.2.193) - 2023-10-06

### Other
- update dependencies

## [0.2.194](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.193...sn_testnet-v0.2.194) - 2023-10-08

### Other
- update dependencies

## [0.2.195](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.194...sn_testnet-v0.2.195) - 2023-10-09

### Other
- update dependencies

## [0.2.196](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.195...sn_testnet-v0.2.196) - 2023-10-09

### Other
- update dependencies

## [0.2.197](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.196...sn_testnet-v0.2.197) - 2023-10-10

### Other
- update dependencies

## [0.2.198](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.197...sn_testnet-v0.2.198) - 2023-10-10

### Other
- update dependencies

## [0.2.199](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.198...sn_testnet-v0.2.199) - 2023-10-10

### Other
- update dependencies

## [0.2.200](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.199...sn_testnet-v0.2.200) - 2023-10-10

### Other
- feature-gating subscription to gossipsub payments notifications

## [0.2.201](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.200...sn_testnet-v0.2.201) - 2023-10-11

### Other
- update dependencies

## [0.2.202](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.201...sn_testnet-v0.2.202) - 2023-10-11

### Other
- update dependencies

## [0.2.203](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.202...sn_testnet-v0.2.203) - 2023-10-11

### Other
- update dependencies

## [0.2.204](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.203...sn_testnet-v0.2.204) - 2023-10-11

### Other
- update dependencies

## [0.2.205](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.204...sn_testnet-v0.2.205) - 2023-10-11

### Other
- update dependencies

## [0.2.206](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.205...sn_testnet-v0.2.206) - 2023-10-12

### Other
- update dependencies

## [0.2.207](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.206...sn_testnet-v0.2.207) - 2023-10-12

### Other
- update dependencies

## [0.2.208](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.207...sn_testnet-v0.2.208) - 2023-10-12

### Other
- update dependencies

## [0.2.209](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.208...sn_testnet-v0.2.209) - 2023-10-13

### Other
- update dependencies

## [0.2.210](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.209...sn_testnet-v0.2.210) - 2023-10-13

### Other
- update dependencies

## [0.2.211](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.210...sn_testnet-v0.2.211) - 2023-10-16

### Other
- update dependencies

## [0.2.212](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.211...sn_testnet-v0.2.212) - 2023-10-16

### Other
- update dependencies

## [0.2.213](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.212...sn_testnet-v0.2.213) - 2023-10-17

### Other
- update dependencies

## [0.2.214](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.213...sn_testnet-v0.2.214) - 2023-10-18

### Other
- update dependencies

## [0.2.215](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.214...sn_testnet-v0.2.215) - 2023-10-18

### Other
- update dependencies

## [0.2.216](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.215...sn_testnet-v0.2.216) - 2023-10-18

### Other
- update dependencies

## [0.2.217](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.216...sn_testnet-v0.2.217) - 2023-10-19

### Other
- update dependencies

## [0.2.218](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.217...sn_testnet-v0.2.218) - 2023-10-19

### Other
- update dependencies

## [0.2.219](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.218...sn_testnet-v0.2.219) - 2023-10-19

### Other
- update dependencies

## [0.2.220](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.219...sn_testnet-v0.2.220) - 2023-10-20

### Other
- update dependencies

## [0.2.221](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.220...sn_testnet-v0.2.221) - 2023-10-20

### Other
- update dependencies

## [0.2.222](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.221...sn_testnet-v0.2.222) - 2023-10-21

### Other
- update dependencies

## [0.2.223](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.222...sn_testnet-v0.2.223) - 2023-10-22

### Other
- update dependencies

## [0.2.224](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.223...sn_testnet-v0.2.224) - 2023-10-23

### Other
- update dependencies

## [0.2.225](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.224...sn_testnet-v0.2.225) - 2023-10-23

### Other
- update dependencies

## [0.2.226](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.225...sn_testnet-v0.2.226) - 2023-10-23

### Other
- update dependencies

## [0.2.227](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.226...sn_testnet-v0.2.227) - 2023-10-23

### Other
- update dependencies

## [0.2.228](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.227...sn_testnet-v0.2.228) - 2023-10-23

### Other
- update dependencies

## [0.2.229](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.228...sn_testnet-v0.2.229) - 2023-10-24

### Other
- update dependencies

## [0.2.230](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.229...sn_testnet-v0.2.230) - 2023-10-24

### Other
- update dependencies

## [0.2.231](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.230...sn_testnet-v0.2.231) - 2023-10-24

### Other
- update dependencies

## [0.2.232](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.231...sn_testnet-v0.2.232) - 2023-10-24

### Other
- update dependencies

## [0.2.233](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.232...sn_testnet-v0.2.233) - 2023-10-24

### Other
- nodes to subscribe by default to network royalties payment notifs

## [0.2.234](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.233...sn_testnet-v0.2.234) - 2023-10-24

### Other
- update dependencies

## [0.2.235](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.234...sn_testnet-v0.2.235) - 2023-10-25

### Other
- update dependencies

## [0.2.236](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.235...sn_testnet-v0.2.236) - 2023-10-26

### Other
- update dependencies

## [0.2.237](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.236...sn_testnet-v0.2.237) - 2023-10-26

### Other
- update dependencies

## [0.2.238](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.237...sn_testnet-v0.2.238) - 2023-10-26

### Fixed
- add libp2p identity with rand dep for tests

## [0.2.239](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.238...sn_testnet-v0.2.239) - 2023-10-26

### Other
- update dependencies

## [0.2.240](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.239...sn_testnet-v0.2.240) - 2023-10-27

### Other
- update dependencies

## [0.2.241](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.240...sn_testnet-v0.2.241) - 2023-10-27

### Other
- update dependencies

## [0.2.242](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.241...sn_testnet-v0.2.242) - 2023-10-27

### Other
- update dependencies

## [0.2.243](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.242...sn_testnet-v0.2.243) - 2023-10-30

### Other
- update dependencies

## [0.2.244](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.243...sn_testnet-v0.2.244) - 2023-10-30

### Other
- update dependencies

## [0.2.245](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.244...sn_testnet-v0.2.245) - 2023-10-30

### Other
- update dependencies

## [0.2.246](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.245...sn_testnet-v0.2.246) - 2023-10-31

### Other
- update dependencies

## [0.2.247](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.246...sn_testnet-v0.2.247) - 2023-10-31

### Other
- update dependencies

## [0.2.248](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.247...sn_testnet-v0.2.248) - 2023-10-31

### Other
- update dependencies

## [0.2.249](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.248...sn_testnet-v0.2.249) - 2023-11-01

### Other
- update dependencies

## [0.2.250](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.249...sn_testnet-v0.2.250) - 2023-11-01

### Other
- update dependencies

## [0.2.251](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.250...sn_testnet-v0.2.251) - 2023-11-01

### Other
- update dependencies

## [0.2.252](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.251...sn_testnet-v0.2.252) - 2023-11-01

### Other
- update dependencies

## [0.2.253](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.252...sn_testnet-v0.2.253) - 2023-11-01

### Other
- update dependencies

## [0.2.254](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.253...sn_testnet-v0.2.254) - 2023-11-02

### Other
- update dependencies

## [0.2.255](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.254...sn_testnet-v0.2.255) - 2023-11-02

### Other
- update dependencies

## [0.2.256](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.255...sn_testnet-v0.2.256) - 2023-11-03

### Other
- update dependencies

## [0.2.257](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.256...sn_testnet-v0.2.257) - 2023-11-03

### Other
- update dependencies

## [0.2.258](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.257...sn_testnet-v0.2.258) - 2023-11-06

### Added
- *(deps)* upgrade libp2p to 0.53

## [0.2.259](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.258...sn_testnet-v0.2.259) - 2023-11-06

### Other
- update dependencies

## [0.2.260](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.259...sn_testnet-v0.2.260) - 2023-11-06

### Other
- update dependencies

## [0.2.261](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.260...sn_testnet-v0.2.261) - 2023-11-06

### Other
- update dependencies

## [0.2.262](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.261...sn_testnet-v0.2.262) - 2023-11-07

### Fixed
- CI errors

## [0.2.263](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.262...sn_testnet-v0.2.263) - 2023-11-07

### Other
- update dependencies

## [0.2.264](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.263...sn_testnet-v0.2.264) - 2023-11-07

### Other
- update dependencies

## [0.2.265](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.264...sn_testnet-v0.2.265) - 2023-11-07

### Other
- update dependencies

## [0.2.266](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.265...sn_testnet-v0.2.266) - 2023-11-08

### Other
- update dependencies

## [0.2.267](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.266...sn_testnet-v0.2.267) - 2023-11-08

### Other
- update dependencies

## [0.2.268](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.267...sn_testnet-v0.2.268) - 2023-11-08

### Other
- update dependencies

## [0.2.269](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.268...sn_testnet-v0.2.269) - 2023-11-09

### Other
- update dependencies

## [0.2.270](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.269...sn_testnet-v0.2.270) - 2023-11-09

### Other
- update dependencies

## [0.2.271](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.270...sn_testnet-v0.2.271) - 2023-11-09

### Other
- update dependencies

## [0.2.272](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.271...sn_testnet-v0.2.272) - 2023-11-10

### Other
- update dependencies

## [0.2.273](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.272...sn_testnet-v0.2.273) - 2023-11-10

### Other
- update dependencies

## [0.2.274](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.273...sn_testnet-v0.2.274) - 2023-11-13

### Other
- update dependencies

## [0.2.275](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.274...sn_testnet-v0.2.275) - 2023-11-13

### Other
- update dependencies

## [0.2.276](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.275...sn_testnet-v0.2.276) - 2023-11-13

### Other
- update dependencies

## [0.2.277](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.276...sn_testnet-v0.2.277) - 2023-11-13

### Other
- update dependencies

## [0.2.278](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.277...sn_testnet-v0.2.278) - 2023-11-14

### Other
- update dependencies

## [0.2.279](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.278...sn_testnet-v0.2.279) - 2023-11-14

### Other
- update dependencies

## [0.2.280](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.279...sn_testnet-v0.2.280) - 2023-11-14

### Other
- update dependencies

## [0.2.281](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.280...sn_testnet-v0.2.281) - 2023-11-14

### Other
- update dependencies

## [0.2.282](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.281...sn_testnet-v0.2.282) - 2023-11-14

### Other
- update dependencies

## [0.2.283](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.282...sn_testnet-v0.2.283) - 2023-11-15

### Other
- update dependencies

## [0.2.284](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.283...sn_testnet-v0.2.284) - 2023-11-15

### Other
- update dependencies

## [0.2.285](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.284...sn_testnet-v0.2.285) - 2023-11-15

### Other
- update dependencies

## [0.2.286](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.285...sn_testnet-v0.2.286) - 2023-11-16

### Other
- update dependencies

## [0.2.287](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.286...sn_testnet-v0.2.287) - 2023-11-16

### Other
- update dependencies

## [0.2.288](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.287...sn_testnet-v0.2.288) - 2023-11-17

### Other
- update dependencies

## [0.2.289](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.288...sn_testnet-v0.2.289) - 2023-11-17

### Other
- update dependencies

## [0.2.290](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.289...sn_testnet-v0.2.290) - 2023-11-20

### Other
- update dependencies

## [0.2.291](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.290...sn_testnet-v0.2.291) - 2023-11-20

### Other
- update dependencies

## [0.2.292](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.291...sn_testnet-v0.2.292) - 2023-11-20

### Other
- update dependencies

## [0.2.293](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.292...sn_testnet-v0.2.293) - 2023-11-20

### Other
- update dependencies

## [0.2.294](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.293...sn_testnet-v0.2.294) - 2023-11-20

### Other
- update dependencies

## [0.2.295](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.294...sn_testnet-v0.2.295) - 2023-11-20

### Other
- update dependencies

## [0.2.296](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.295...sn_testnet-v0.2.296) - 2023-11-21

### Other
- update dependencies

## [0.2.297](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.296...sn_testnet-v0.2.297) - 2023-11-21

### Other
- update dependencies

## [0.2.298](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.297...sn_testnet-v0.2.298) - 2023-11-22

### Other
- update dependencies

## [0.2.299](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.298...sn_testnet-v0.2.299) - 2023-11-22

### Other
- update dependencies

## [0.2.300](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.299...sn_testnet-v0.2.300) - 2023-11-22

### Other
- update dependencies

## [0.2.301](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.300...sn_testnet-v0.2.301) - 2023-11-23

### Other
- update dependencies

## [0.2.302](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.301...sn_testnet-v0.2.302) - 2023-11-23

### Other
- update dependencies

## [0.2.303](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.302...sn_testnet-v0.2.303) - 2023-11-23

### Other
- update dependencies

## [0.2.304](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.303...sn_testnet-v0.2.304) - 2023-11-23

### Other
- update dependencies

## [0.2.305](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.304...sn_testnet-v0.2.305) - 2023-11-24

### Other
- update dependencies

## [0.2.306](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.305...sn_testnet-v0.2.306) - 2023-11-27

### Other
- update dependencies

## [0.2.307](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.306...sn_testnet-v0.2.307) - 2023-11-28

### Other
- update dependencies

## [0.2.308](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.307...sn_testnet-v0.2.308) - 2023-11-28

### Other
- update dependencies

## [0.2.309](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.308...sn_testnet-v0.2.309) - 2023-11-28

### Other
- update dependencies

## [0.2.310](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.309...sn_testnet-v0.2.310) - 2023-11-29

### Other
- update dependencies

## [0.2.311](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.310...sn_testnet-v0.2.311) - 2023-11-29

### Other
- update dependencies

## [0.2.312](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.311...sn_testnet-v0.2.312) - 2023-11-29

### Other
- update dependencies

## [0.2.313](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.312...sn_testnet-v0.2.313) - 2023-11-29

### Other
- update dependencies

## [0.2.314](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.313...sn_testnet-v0.2.314) - 2023-11-29

### Other
- update dependencies

## [0.2.315](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.314...sn_testnet-v0.2.315) - 2023-11-29

### Other
- update dependencies

## [0.2.316](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.315...sn_testnet-v0.2.316) - 2023-12-01

### Other
- update dependencies

## [0.2.317](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.316...sn_testnet-v0.2.317) - 2023-12-04

### Added
- *(testnet)* wait till faucet server starts

### Fixed
- *(testnet)* dont be taking faucet stdout

## [0.2.318](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.317...sn_testnet-v0.2.318) - 2023-12-05

### Other
- *(networking)* remove triggered bootstrap slowdown

## [0.2.319](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.318...sn_testnet-v0.2.319) - 2023-12-05

### Other
- update dependencies

## [0.2.320](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.319...sn_testnet-v0.2.320) - 2023-12-05

### Other
- update dependencies

## [0.2.321](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.320...sn_testnet-v0.2.321) - 2023-12-05

### Other
- update dependencies

## [0.2.322](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.321...sn_testnet-v0.2.322) - 2023-12-05

### Other
- update dependencies

## [0.2.323](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.322...sn_testnet-v0.2.323) - 2023-12-05

### Other
- update dependencies

## [0.2.324](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.323...sn_testnet-v0.2.324) - 2023-12-06

### Other
- add more workspace lints from node
- remove needless pass by value
- forbid unsafe idioms at workspace level
- use inline format args
- add boilerplate for workspace lints

## [0.2.325](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.324...sn_testnet-v0.2.325) - 2023-12-06

### Other
- update dependencies

## [0.2.326](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.325...sn_testnet-v0.2.326) - 2023-12-06

### Other
- update dependencies

## [0.2.327](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.326...sn_testnet-v0.2.327) - 2023-12-07

### Other
- update dependencies

## [0.2.328](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.327...sn_testnet-v0.2.328) - 2023-12-08

### Other
- update dependencies

## [0.2.329](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.328...sn_testnet-v0.2.329) - 2023-12-08

### Other
- update dependencies

## [0.2.330](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.329...sn_testnet-v0.2.330) - 2023-12-08

### Other
- update dependencies

## [0.2.331](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.330...sn_testnet-v0.2.331) - 2023-12-11

### Other
- update dependencies

## [0.2.332](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.331...sn_testnet-v0.2.332) - 2023-12-11

### Other
- update dependencies

## [0.2.333](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.332...sn_testnet-v0.2.333) - 2023-12-12

### Other
- update dependencies

## [0.2.334](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.333...sn_testnet-v0.2.334) - 2023-12-12

### Other
- update dependencies

## [0.2.335](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.334...sn_testnet-v0.2.335) - 2023-12-12

### Other
- update dependencies

## [0.2.336](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.335...sn_testnet-v0.2.336) - 2023-12-12

### Other
- update dependencies

## [0.2.337](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.336...sn_testnet-v0.2.337) - 2023-12-12

### Other
- update dependencies

## [0.2.338](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.337...sn_testnet-v0.2.338) - 2023-12-13

### Other
- update dependencies

## [0.2.339](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.338...sn_testnet-v0.2.339) - 2023-12-13

### Other
- update dependencies

## [0.2.340](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.339...sn_testnet-v0.2.340) - 2023-12-13

### Other
- update dependencies

## [0.2.341](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.340...sn_testnet-v0.2.341) - 2023-12-13

### Other
- update dependencies

## [0.2.342](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.341...sn_testnet-v0.2.342) - 2023-12-14

### Other
- update dependencies

## [0.2.343](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.342...sn_testnet-v0.2.343) - 2023-12-14

### Other
- update dependencies

## [0.2.344](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.343...sn_testnet-v0.2.344) - 2023-12-14

### Other
- update dependencies

## [0.2.345](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.344...sn_testnet-v0.2.345) - 2023-12-14

### Other
- update dependencies

## [0.2.346](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.345...sn_testnet-v0.2.346) - 2023-12-14

### Other
- update dependencies

## [0.2.347](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.346...sn_testnet-v0.2.347) - 2023-12-14

### Other
- update dependencies

## [0.2.348](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.347...sn_testnet-v0.2.348) - 2023-12-18

### Other
- update dependencies

## [0.2.349](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.348...sn_testnet-v0.2.349) - 2023-12-18

### Other
- update dependencies

## [0.2.350](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.349...sn_testnet-v0.2.350) - 2023-12-18

### Other
- update dependencies

## [0.2.351](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.350...sn_testnet-v0.2.351) - 2023-12-18

### Other
- update dependencies

## [0.2.352](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.351...sn_testnet-v0.2.352) - 2023-12-19

### Other
- update dependencies

## [0.2.353](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.352...sn_testnet-v0.2.353) - 2023-12-19

### Other
- update dependencies

## [0.2.354](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.353...sn_testnet-v0.2.354) - 2023-12-19

### Other
- update dependencies

## [0.2.355](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.354...sn_testnet-v0.2.355) - 2023-12-19

### Other
- update dependencies

## [0.2.356](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.355...sn_testnet-v0.2.356) - 2023-12-19

### Other
- add data path field to node info

## [0.2.357](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.356...sn_testnet-v0.2.357) - 2023-12-20

### Other
- update dependencies

## [0.2.358](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.357...sn_testnet-v0.2.358) - 2023-12-21

### Other
- update dependencies

## [0.2.359](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.358...sn_testnet-v0.2.359) - 2023-12-21

### Other
- update dependencies

## [0.2.360](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.359...sn_testnet-v0.2.360) - 2023-12-22

### Other
- update dependencies

## [0.2.361](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.360...sn_testnet-v0.2.361) - 2023-12-22

### Other
- update dependencies

## [0.2.362](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.361...sn_testnet-v0.2.362) - 2023-12-26

### Other
- update dependencies

## [0.2.363](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.362...sn_testnet-v0.2.363) - 2023-12-29

### Other
- update dependencies

## [0.2.364](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.363...sn_testnet-v0.2.364) - 2023-12-29

### Other
- update dependencies

## [0.2.365](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.364...sn_testnet-v0.2.365) - 2023-12-29

### Other
- update dependencies

## [0.2.366](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.365...sn_testnet-v0.2.366) - 2024-01-02

### Other
- update dependencies

## [0.2.367](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.366...sn_testnet-v0.2.367) - 2024-01-02

### Other
- update dependencies

## [0.2.368](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.367...sn_testnet-v0.2.368) - 2024-01-03

### Other
- update dependencies

## [0.2.369](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.368...sn_testnet-v0.2.369) - 2024-01-03

### Other
- update dependencies

## [0.2.370](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.369...sn_testnet-v0.2.370) - 2024-01-03

### Other
- update dependencies

## [0.2.371](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.370...sn_testnet-v0.2.371) - 2024-01-04

### Other
- update dependencies

## [0.2.372](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.371...sn_testnet-v0.2.372) - 2024-01-04

### Other
- update dependencies

## [0.2.373](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.372...sn_testnet-v0.2.373) - 2024-01-05

### Other
- update dependencies

## [0.2.374](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.373...sn_testnet-v0.2.374) - 2024-01-05

### Other
- update dependencies

## [0.2.375](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.374...sn_testnet-v0.2.375) - 2024-01-05

### Other
- update dependencies

## [0.2.376](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.375...sn_testnet-v0.2.376) - 2024-01-05

### Other
- update dependencies

## [0.2.377](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.376...sn_testnet-v0.2.377) - 2024-01-05

### Fixed
- ignore unwraps in protogen files

## [0.2.378](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.377...sn_testnet-v0.2.378) - 2024-01-05

### Other
- update dependencies

## [0.2.379](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.378...sn_testnet-v0.2.379) - 2024-01-06

### Other
- update dependencies

## [0.2.380](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.379...sn_testnet-v0.2.380) - 2024-01-08

### Other
- update dependencies

## [0.2.381](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.380...sn_testnet-v0.2.381) - 2024-01-08

### Other
- update dependencies

## [0.2.382](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.381...sn_testnet-v0.2.382) - 2024-01-08

### Other
- update dependencies

## [0.2.383](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.382...sn_testnet-v0.2.383) - 2024-01-08

### Other
- update dependencies

## [0.3.0](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.2.383...sn_testnet-v0.3.0) - 2024-01-08

### Added
- provide `--first` argument for `safenode`

## [0.3.1](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.3.0...sn_testnet-v0.3.1) - 2024-01-09

### Other
- update dependencies

## [0.3.2](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.3.1...sn_testnet-v0.3.2) - 2024-01-09

### Other
- update dependencies

## [0.3.3](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.3.2...sn_testnet-v0.3.3) - 2024-01-09

### Other
- update dependencies

## [0.3.4](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.3.3...sn_testnet-v0.3.4) - 2024-01-09

### Other
- update dependencies

## [0.3.5](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.3.4...sn_testnet-v0.3.5) - 2024-01-10

### Other
- update dependencies

## [0.3.6](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.3.5...sn_testnet-v0.3.6) - 2024-01-10

### Fixed
- removal of exe for agnostic cmds in readme

## [0.3.7](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.3.6...sn_testnet-v0.3.7) - 2024-01-11

### Other
- update dependencies

## [0.3.8](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.3.7...sn_testnet-v0.3.8) - 2024-01-11

### Other
- update dependencies

## [0.3.9](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.3.8...sn_testnet-v0.3.9) - 2024-01-11

### Other
- update dependencies

## [0.3.10](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.3.9...sn_testnet-v0.3.10) - 2024-01-11

### Other
- update dependencies

## [0.3.11](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.3.10...sn_testnet-v0.3.11) - 2024-01-12

### Other
- update dependencies

## [0.3.12](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.3.11...sn_testnet-v0.3.12) - 2024-01-12

### Other
- update dependencies

## [0.3.13](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.3.12...sn_testnet-v0.3.13) - 2024-01-15

### Other
- update dependencies

## [0.3.14](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.3.13...sn_testnet-v0.3.14) - 2024-01-15

### Other
- update dependencies

## [0.3.15](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.3.14...sn_testnet-v0.3.15) - 2024-01-15

### Other
- update dependencies

## [0.3.16](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.3.15...sn_testnet-v0.3.16) - 2024-01-15

### Other
- update dependencies

## [0.3.17](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.3.16...sn_testnet-v0.3.17) - 2024-01-16

### Other
- update dependencies

## [0.3.18](https://github.com/maidsafe/safe_network/compare/sn_testnet-v0.3.17...sn_testnet-v0.3.18) - 2024-01-16

### Other
- update dependencies

## v0.1.0 (2023-03-16)

<csr-id-4f04bd1a5d1c747bfc6b5d39824dd108f8546b7b/>
<csr-id-1c621d13b5edfc21ed85da7498d24c5db038795a/>

### Chore

 - <csr-id-4f04bd1a5d1c747bfc6b5d39824dd108f8546b7b/> rename testnet crate to sn_testnet
   Even though the `testnet` crate name is not taken on crates.io, I think it makes sense to prefix
   this crate with `sn_`, as per our other crates. The name of the binary does not change. This crate
   needs to be published because `sn_client` has a dependency on it.
   
   This also provides a README for the crate, which was necessary to have it published.

### Other

 - <csr-id-1c621d13b5edfc21ed85da7498d24c5db038795a/> temporarily prevent workflows running
   I want to temporarily disable the version bump and release workflows from running so that I can
   manually publish the new testnet crate and delete the tags from the last bad release.

