# Noosphere
Core implementation.

## Build Notes

1. To build, make sure you have the latest rust build environment:
https://rustup.rs/
2. You will also need a web driver (e.g. [ChromeDriver](https://chromedriver.chromium.org/getting-started) )
3. `cargo build`
4. To run tests `cargo test`
5. Install bindgen: `cargo install -f wasm-bindgen-cli` and update `cargo update -p wasm-bindgen`
6. Now run tests from wasm target: `CHROMEDRIVER=$CHROMEDRIVER_PATH cargo test --target wasm32-unknown-unknown`
