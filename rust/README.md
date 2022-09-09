# Noosphere

Core implementation.

## Build notes

You may need OpenSSL development files to be installed locally:

```sh
sudo apt install libssl-dev
```

1. To build, make sure you have the latest rust build environment:
   https://rustup.rs/
2. You will also need a web driver (e.g. [ChromeDriver](https://chromedriver.chromium.org/getting-started) )
3. `cargo build`
4. To run tests `cargo test`
5. Install bindgen: `cargo install -f wasm-bindgen-cli` and update `cargo update -p wasm-bindgen`
6. Now run tests from wasm target: `CHROMEDRIVER=$CHROMEDRIVER_PATH cargo test --target wasm32-unknown-unknown`

## Editor notes

Rust analyzer may have issues expanding `#[async_trait]`:

- async_trait https://github.com/rust-lang/rust-analyzer/issues/11533#issuecomment-1048439468
