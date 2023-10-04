# Safe Network Metrics
Collect and visualize metrics from Safe Network nodes using Prometheus and Grafana.

### Prerequisites
1. **Docker & Docker-Compose:** Ensure you have both docker and docker-compose installed on your system.
2. **Safe Node Configuration:** When running your Safe nodes, ensure they are started with the `--feature=open-metrics` flag enabled.

### Usage

If the nodes are started with the `open-metrics` feature, then the URL to the metrics server are written to the log files. Provide the `[log_dir_path]...` to scan the logs to obtain the server URLs. If `[log_dir_path]...` is not provided, it defaults to `data-dir` log that the nodes use by default.

```bash
cargo run --release --bin metrics -- [log_dir_path]...
```

After running the above command, Prometheus and Grafana containers will start automatically. And the Grafana dashboard URL will be printed to the console. Access this URL to visualize your node metrics.