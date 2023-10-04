# Safe Network Metrics

Prerequisite:
1. Install `docker-compose`
2. Run safe nodes with `--feature=open-metrics` enabled

Run `cargo run --release optional<log_dir_path>` to scan all the log files under the provided path and provision
prometheus with the node metrics servers. This would then start the prometheus-grafana containers and print the
grafana dashboard URL.

The `log_dir_path` defaults to `data-dir` if it is not provided.