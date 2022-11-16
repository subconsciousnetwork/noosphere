![API Stability: Alpha](https://img.shields.io/badge/API%20Stability-Alpha-red)

# noosphere-ns

Noosphere's P2P name system.

## Bootstrap Node

The `bootstrap_nns` binary target is an executable that runs one or many bootstrap
nodes, based on configuration.

```
cargo run --bin bootstrap_nns -- run --key my-key-name --port 6666
```