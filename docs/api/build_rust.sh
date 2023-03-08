#!/usr/bin/env bash

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)
TARGET_DOCS="$SCRIPT_DIR/../../target/doc"
RUST_SRC="$SCRIPT_DIR/../../rust/noosphere"
OUT_DOCS="$SCRIPT_DIR/../out/rust"
cd $RUST_SRC
rm -rf $TARGET_DOCS
cargo doc
cp -r $TARGET_DOCS $OUT_DOCS
