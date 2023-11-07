# Safe Network Faucet
This is a command line application that allows you to run a Safe Network Faucet.

## Usage
Run `cargo run -- <command>` to start the application. Some of the commands available are:

- `ClaimGenesis`: Claim the amount in the genesis CashNote and deposit it to the faucet local wallet.
- `Send`: Send a specified amount of tokens to a specified wallet.
- `Server`: Starts an http server that will send tokens to anyone who requests them.

For more information about each command, run `cargo run -- <command> --help`.
