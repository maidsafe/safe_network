# sn_auditor

This is a small webserver application that allows you to audit the SAFE Network Currency by gathering a DAG of Spends on the Network. 

![](./resources/dag.svg)

## Usage

Running an auditor instance:

```bash
# on a Network with known peers
cargo run --release --peer "/ip4/<network_peer_addr>"

# on a local testnet
cargo run --release --features=local
```

It can be run with the following flags:

```bash
  -f, --force-from-genesis
          Force the spend DAG to be updated from genesis

  -c, --clean
          Clear the local spend DAG and start from scratch

  -o, --offline-viewer <dag_file>
          Visualize a local DAG file offline, does not connect to the Network

  -b, --beta-participants <discord_names_file>
          Beta rewards program participants to track
          Provide a file with a list of Discord
          usernames as argument

  -k, --beta-encryption-key <hex_secret_key>
          Secret encryption key of the beta rewards to decypher
          discord usernames of the beta participants
```

The following env var:

```
# time in seconds UTXOs are refetched in DAG crawl
UTXO_REATTEMPT_INTERVAL=3600
```

## Endpoints

The webserver listens on port `4242` and has the following endpoints:

| route             | description                                       |
|-------------------|---------------------------------------------------|
|`"/"`              | `svg` representation of the DAG                   |
|`"/spend/<addr>"`  | `json` information about the spend at this `addr` |
|`"/beta-rewards"`  | `json` list of beta rewards participants          |

Note that for the `"/"` endpoint to work properly you need:
- to have [graphviz](https://graphviz.org/download/) installed
- to enable the `svg-dag` feature flag (with `cargo run --release --features=svg-dag`)
