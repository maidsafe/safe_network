# Safe Network Metrics Dashboard
Easily visualize metrics from Safe Network nodes using Prometheus and Grafana. This guide covers the steps to set up the dashboard and terminate it.

### Prerequisites
1. **Docker & Docker-Compose:** Ensure you have both `docker` and `docker-compose` installed on your system and make sure they're running.
2. **Safe Node Configuration:** When running your Safe nodes, ensure they are started with the `--feature=open-metrics` flag.

### 1. Start the Dashboard:

#### Manual Start:
Run the following command to scan the log files and fetch the metrics server URLs. These URLs will be used to create the Prometheus config file.

```bash
cargo run --release --bin metrics -- [log_dir_path]...
```
Note: If [log_dir_path]... is not provided, it will default to the `data-dir` log that nodes use by default.

The above command would write the Prometheus config file to `./metrics/prometheus/prometheus.yml`

- Navigate to the metrics directory:
```bash
cd metrics
```
- Start the containers:
```bash
docker-compose up --detach
```

#### Automated Start:
Run the binary with the `--run` flag to fetch the configuration file and automatically start the containers:

```bash
cargo run --release --bin metrics -- [log_dir_path]... --run
```

### 2. Access the Dashboard:
Once started, access the Grafana dashboard at: http://localhost:3001/d/node_metrics/node-metrics?orgId=1&refresh=5s

Login Credentials:
```makefile
username: admin
password: pwd
```

### 3. Terminate the Dashboard:
To stop the containers and clear all the data:

```bash
cd metrics
docker-compose down --volumes
```