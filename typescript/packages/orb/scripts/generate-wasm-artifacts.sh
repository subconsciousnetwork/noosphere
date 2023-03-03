#!/bin/bash

set -e
# See: https://stackoverflow.com/a/246128
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
CARGO_WORKSPACE_DIR="$SCRIPT_DIR/../../../.."
NOOSPHERE_DIR="$CARGO_WORKSPACE_DIR/rust/noosphere"
CARGO_TARGET_DIR="$CARGO_WORKSPACE_DIR/target"
CARGO_TARGET_NOOSPHERE_WASM="$CARGO_TARGET_DIR/wasm32-unknown-unknown/release/noosphere.wasm"
ARTIFACT_OUTPUT_DIR="$SCRIPT_DIR/../lib"

# Build Wasm target from Rust crates
pushd $NOOSPHERE_DIR
cargo build --release --target wasm32-unknown-unknown --features ipfs-storage
popd

# Generate web artifacts, including TypeScript types and JS shims
mkdir -p $ARTIFACT_OUTPUT_DIR
wasm-bindgen $CARGO_TARGET_NOOSPHERE_WASM --out-dir $ARTIFACT_OUTPUT_DIR --target web

# Optimize the Wasm blob; this step reduces the size by ~25%
pushd $ARTIFACT_OUTPUT_DIR
wasm-opt -Oz --vacuum --strip-debug ./noosphere_bg.wasm -o ./noosphere_bg.wasm
popd

set +e
