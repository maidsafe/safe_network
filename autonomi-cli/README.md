# A CLI for the Autonomi Network

```
Usage: autonomi [OPTIONS] <COMMAND>

Commands:
  file      Operations related to file handling
  register  Operations related to register management
  vault     Operations related to vault management
  wallet    Operations related to wallet management
  help      Print this message or the help of the given subcommand(s)

Options:
      --log-output-dest <LOG_OUTPUT_DEST>
          Specify the logging output destination. [default: data-dir]
      --log-format <LOG_FORMAT>
          Specify the logging format.
      --peer <multiaddr>
          Peer(s) to use for bootstrap, in a 'multiaddr' format containing the peer ID [env: SAFE_PEERS=]
      --timeout <CONNECTION_TIMEOUT>
          The maximum duration to wait for a connection to the network before timing out
  -x, --no-verify
          Prevent verification of data storage on the network
  -h, --help
          Print help (see more with '--help')
  -V, --version
          Print version
```

## Wallet

### Create a new wallet

```bash
wallet create
```

> Add the `--no-password` flag to skip the optional encryption step.

> **Wallet Security**
>
> Encrypted wallets provide an additional layer of security, requiring a password to read the private key and perform
> transactions. However, ensure you remember your password; losing it may result in the inability to access your encrypted
> wallet.

Example:

   ```bash
   $ wallet create
   Enter password (leave empty for none):
   Repeat password:
   Wallet address: 0xaf676aC7C821977506AC9DcE28bFe83fb06938d8
   Stored wallet in: "/Users/macuser/Library/Application Support/autonomi/client/wallets/0xaf676aC7C821977506AC9DcE28bFe83fb06938d8.encrypted"
   ```

### Import a wallet

```bash
wallet create --private-key <PRIVATE_KEY>
```

### Check wallet balance

```bash
wallet balance
```

Example:

   ```bash
   $ wallet balance
   Wallet balances: 0x5A631e17FfB0F07b00D88E0e42246495Bf21d698
   +---------------+---+
   | Token Balance | 0 |
   +---------------+---+
   | Gas Balance   | 0 |
   +---------------+---+
   ```

## License

This Safe Network repository is licensed under the General Public License (GPL), version
3 ([LICENSE](LICENSE) http://www.gnu.org/licenses/gpl-3.0.en.html).

---

Feel free to modify or expand upon this README as needed. Would you like to add or change anything else?
