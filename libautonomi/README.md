# `libautonomi` - Autonomi client API

[![Crates.io](https://img.shields.io/crates/v/libautonomi.svg)](https://crates.io/crates/libautonomi)
[![docs.rs](https://img.shields.io/badge/api-rustdoc-blue.svg)](https://docs.rs/libautonomi)

Connect to and build on the Autonomi network.

## Usage

See [docs.rs/libautonomi](https://docs.rs/libautonomi) for usage examples.

## Running tests

Run a local network with the `local-discovery` feature:

```sh
cargo run --bin=safenode-manager --features=local-discovery -- local run --build --clean
```

Then run the tests with the `local` feature:
```sh
$ cargo test --package=libautonomi --features=local
# Or with logs
$ RUST_LOG=libautonomi cargo test --package=libautonomi --features=local -- --nocapture
```
