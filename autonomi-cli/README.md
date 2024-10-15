# A CLI for the Autonomi Network

```
Usage: autonomi_cli [OPTIONS] <COMMAND>

Commands:
  file      Operations related to file handling
  register  Operations related to register management
  vault     Operations related to vault management
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