# Safenode Manager

Safenode Manager is a command-line application for installing, managing, and operating `safenode` as a service. This tool facilitates easy setup and control of `safenode` services. It runs on Linux, macOS and Windows.

## Installation

As with other Safe-related components, Safenode Manager will shortly be available through the `safeup` application. For now, a binary can be obtained for your platform through the releases in this repository.

## Commands

### Add

- Command: `add`
- Description: Downloads `safenode` and sets up a new service.
- Options:
  - `--count`: Number of service instances to add. Optional. Default: 1.
  - `--data-dir-path`: Path for the data directory. Optional, with platform-specific defaults.
  - `--log-dir-path`: Path for the log directory. Optional, with platform-specific defaults.
  - `--user`: User account under which the service should run. Optional. Default: `safe`.
  - `--version`: Version of `safenode` to add. Optional. Default: the latest version.
- Usage: `safenode-manager add [OPTIONS]`

This command must run as the root user on Linux/macOS and the Administrator user on Windows.

The default location for the node's data directory will be `/var/safenode-manager/services` for Linux and macOS, and `C:\ProgramData\safenode\data` on Windows. Use the `--data-dir-path` argument if you'd like to use an alternate location, perhaps a larger disk you may have mounted.

On Linux and macOS, a non-root user account, `safe`, will be created, and the service will run as this user. If you'd like to use a different user, override with the `--user` argument. This argument will have no effect on Windows, where the service will be running as the `LocalSystem` account.

Nodes will not be started after they are added.

The command can run as many times as you like to repeatedly add more nodes.

### Start

- Command: `start`
- Description: Starts an installed `safenode` service.
- Options:
  - `--peer-id`: Peer ID of the service to start. Optional.
  - `--service-name`: Name of the service to start. Optional.
- Usage: `safenode-manager start [OPTIONS]`

This command must run as the root user on Linux/macOS and the Administrator user on Windows.

Running the command with no arguments will start every installed node that is not already running. The peer ID or service name can be used to start a specific service.

A peer ID will be assigned to a node after it is started for the first time.

### Status

- Command: `status`
- Description: Displays the status of installed services.
- Options:
  - `--details`: Displays more detailed information. Boolean flag.
- Usage: `safenode-manager status [OPTIONS]`

### Stop

- Command: `stop`
- Description: Stops an installed `safenode` service.
- Options:
  - `--peer_id`: Peer ID of the service to stop. Optional.
  - `--service_name`: Name of the service to stop. Optional.
- Usage: `safenode-manager stop [OPTIONS]`

This command must run as the root user on Linux/macOS and the Administrator user on Windows.

Running the command with no arguments will stop every installed node that is not already stopped. The peer ID or service name can be used to start a specific service.

If started again, the node's data and peer ID will be retained.

### Remove

- Command: `remove`
- Description: Removes a `safenode` service.
- Options:
  - `--peer-id`: Peer ID of the service to remove. Optional.
  - `--service-name`: Name of the service to remove. Optional.
  - `--keep-directories`: Set this flag to keep the node's data and log directories. Optional.
- Usage: `safenode-manager remove [OPTIONS]`

This command must run as the root user on Linux/macOS and the Administrator user on Windows.

Removes the node and its data/log directories. The node must be stopped before running this command.

### Upgrade

- Command: `upgrade`
- Description: Upgrades a `safenode` service to the latest version.
- Options:
  - `--peer_id`: Peer ID of the service to stop. Optional.
  - `--service_name`: Name of the service to stop. Optional.
- Usage: `safenode-manager upgrade [OPTIONS]`

This command must run as the root user on Linux/macOS and the Administrator user on Windows.

Running the command with no arguments will upgrade every node. The peer ID or service name can be used to upgrade a specific service.

## License

This Safe Network repository is licensed under the General Public License (GPL), version 3 ([LICENSE](LICENSE) http://www.gnu.org/licenses/gpl-3.0.en.html).

See the [LICENSE](LICENSE) file for more details.
