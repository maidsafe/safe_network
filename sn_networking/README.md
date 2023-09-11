# sn_networking

Defines the core networking infrastructure for the Safe Network, which is based around the [libp2p](https://github.com/libp2p) stack.

# Network Metrics
Use the `network-metrics` feature flag on the node/client to enable [libp2p metrics](https://docs.rs/libp2p-metrics/latest/libp2p_metrics/). This records libp2p protocol / Swarm events and exposes them in the [OpenMetrics](https://github.com/OpenObservability/OpenMetrics/) format. This can be imported inside a Grafana dashboard, as seen [here](https://kademlia-exporter.max-inden.de/d/Pfr0Fj6Mk/rust-libp2p?orgId=1&refresh=30s).

A webserver is run per node that will expose the metrics recorded by that node. Check the info level log to find webserver's port, `Metrics server on http://127.0.0.1:xxxx/metrics`
