# Storage Benchmarks

Benchmarking suite for comparing [Storage] providers.

## SledStorage

```
cargo run
```

## RocksDbStorage

```
cargo run --features rocksdb
```

## IndexedDbStorage

Open up a browser to `http://localhost:8000`

```
NO_HEADLESS=1 cargo run --target wasm32-unknown-unknown
```


