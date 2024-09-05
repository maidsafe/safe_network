# `autonomi` - Autonomi client API

[![Crates.io](https://img.shields.io/crates/v/autonomi.svg)](https://crates.io/crates/autonomi)
[![docs.rs](https://img.shields.io/badge/api-rustdoc-blue.svg)](https://docs.rs/autonomi)

Connect to and build on the Autonomi network.

## Usage

See [docs.rs/autonomi](https://docs.rs/autonomi) for usage examples.

## Running tests

Run a local network with the `local-discovery` feature:

```sh
cargo run --bin=safenode-manager --features=local-discovery -- local run --build --clean
```

Then run the tests with the `local` feature:
```sh
$ cargo test --package=autonomi --features=local
# Or with logs
$ RUST_LOG=autonomi cargo test --package=autonomi --features=local -- --nocapture
```
