# stableset_net

## Running it

```bash
killall safenode || true && rm ./peers.json || true && cargo run --bin testnet -- -b --interval 100
```

Nodes find their peers in the `peers.json` file. If this doesnt exist, the first node to start will write their ip there. 
Every node usees a random port, and should eventually discover all ndoes.